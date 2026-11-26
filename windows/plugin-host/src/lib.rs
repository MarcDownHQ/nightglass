//! Providers compiled to WebAssembly components against `wit/provider.wit`. Every call runs in a
//! fresh instance with no files, sockets or environment: HTTP goes through `Http::send`.

mod convert;
mod storage;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nightglass_core::{BoxFuture, Catalog, Channel, Config, Episode, Http, IptvError, LicenseRequest, Media, Movie, Playback, Provider, ProviderInfo, Providers, Result, Series};
use serde::Serialize;
use url::Url;
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../wit",
        world: "provider",
        imports: { default: async },
        exports: { default: async },
    });
}

use bindings::nightglass::provider::{http as wit_http, log as wit_log, storage as wit_storage, types as wit};
use storage::Storage;

const MEMORY: usize = 256 << 20;
const CALL_TIMEOUT: Duration = Duration::from_secs(300);
const DESCRIBE_TIMEOUT: Duration = Duration::from_secs(10);
/// A segment the player is waiting for.
const FILTER_TIMEOUT: Duration = Duration::from_secs(30);
/// A license the CDM is waiting for, before the player can start.
const LICENSE_TIMEOUT: Duration = Duration::from_secs(30);
const EPOCH: Duration = Duration::from_millis(10);

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
    http: Http,
    storage: Arc<Storage>,
    plugin: String,
    /// The provider id once `describe` has given it: storage is keyed by it, and refused before.
    keys: Option<String>,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl wit::Host for State {}

impl wit_http::Host for State {
    async fn send(&mut self, request: wit_http::Request) -> std::result::Result<wit_http::Response, wit::Error> {
        let url = Url::parse(&request.url).map_err(|_| wit::Error::Network(format!("invalid URL {}", request.url)))?;
        let response = self.http.send(nightglass_core::Request { method: request.method, url, headers: request.headers, body: request.body, browser: request.browser.map(convert::browser) }).await.map_err(convert::to_wit_error)?;
        Ok(wit_http::Response { status: response.status, headers: response.headers, body: response.body })
    }
}

impl wit_storage::Host for State {
    async fn get(&mut self, key: String) -> Option<String> {
        let (storage, plugin) = (self.storage.clone(), self.keys.clone()?);
        blocking(move || storage.get(&plugin, &key)).await.flatten()
    }

    async fn set(&mut self, key: String, value: String, ttl: Option<u32>, persist: bool) -> std::result::Result<(), String> {
        let (storage, plugin) = (self.storage.clone(), self.keys.clone().ok_or("no storage during describe")?);
        blocking(move || storage.set(&plugin, key, value, ttl, persist)).await.unwrap_or_else(|| Err("storage failed".into()))
    }

    async fn delete(&mut self, key: String) {
        let Some(plugin) = self.keys.clone() else { return };
        let storage = self.storage.clone();
        blocking(move || storage.delete(&plugin, &key)).await;
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

impl wit_log::Host for State {
    async fn log(&mut self, level: wit_log::Level, message: String) {
        let level = match level {
            wit_log::Level::Debug => log::Level::Debug,
            wit_log::Level::Info => log::Level::Info,
            wit_log::Level::Warn => log::Level::Warn,
            wit_log::Level::Error => log::Level::Error,
        };
        log::log!(level, "plugin {}: {message}", self.plugin);
    }
}

/// Compiles and runs plugins. One per app: it owns the engine and its clock.
pub struct Host {
    engine: Engine,
    linker: Linker<State>,
    http: Http,
    storage: Arc<Storage>,
}

/// What became of one file of the plugins folder.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatus {
    pub file: String,
    pub provider: Option<String>,
    pub error: Option<String>,
}

impl Host {
    pub fn new(http: &Http) -> wasmtime::Result<Self> {
        let mut config = wasmtime::Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        bindings::Provider::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
        let weak = engine.weak();
        std::thread::Builder::new().name("plugin-clock".into()).spawn(move || {
            while let Some(engine) = weak.upgrade() {
                engine.increment_epoch();
                drop(engine);
                std::thread::sleep(EPOCH);
            }
        })?;
        Ok(Host { engine, linker, http: http.clone(), storage: Arc::new(Storage::new(None)) })
    }

    /// Keeps what plugins store with `persist` in `dir`, one file per plugin; otherwise it lasts until the host is dropped.
    pub fn with_data_dir(mut self, dir: PathBuf) -> Self {
        self.storage = Arc::new(Storage::new(Some(dir)));
        self
    }

    /// Adds every `*.wasm` of `dir` to `providers`. A plugin that fails is reported, not fatal.
    pub async fn load_dir(&self, dir: &Path, providers: &mut Providers) -> Vec<PluginStatus> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut files: Vec<_> = entries.filter_map(|e| Some(e.ok()?.path())).filter(|p| p.extension().is_some_and(|e| e == "wasm")).collect();
        files.sort();
        let mut out = Vec::new();
        for path in files {
            let file = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let added = match self.load(&path).await {
                Ok(provider) => {
                    let name = provider.info().name.clone();
                    providers.add(provider).map(|()| name)
                }
                Err(e) => Err(e),
            };
            if let Err(e) = &added {
                log::warn!("plugin {file}: {e}");
            }
            let (provider, error) = match added {
                Ok(name) => (Some(name), None),
                Err(e) => (None, Some(e)),
            };
            out.push(PluginStatus { file, provider, error });
        }
        out
    }

    pub async fn load(&self, path: &Path) -> std::result::Result<Arc<dyn Provider>, String> {
        let component = Component::from_file(&self.engine, path).map_err(|e| format!("{e:#}"))?;
        let pre = self.linker.instantiate_pre(&component).map_err(|e| format!("{e:#}"))?;
        let pre = bindings::ProviderPre::new(pre).map_err(|e| format!("{e:#}"))?;
        let runner = Runner { pre, engine: self.engine.clone(), http: self.http.clone(), storage: self.storage.clone() };
        let file = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let call = async {
            let (mut store, guest) = runner.instance(&file, None).await?;
            guest.call_describe(&mut store).await
        };
        let info = finish(DESCRIBE_TIMEOUT, call, |info| Ok(Ok(info))).await.map_err(|e| e.to_string())?;
        Ok(Arc::new(WasmProvider { info: convert::info(info)?, runner }))
    }
}

struct Runner {
    pre: bindings::ProviderPre<State>,
    engine: Engine,
    http: Http,
    storage: Arc<Storage>,
}

impl Runner {
    async fn instance(&self, plugin: &str, keys: Option<&str>) -> wasmtime::Result<(Store<State>, bindings::Provider)> {
        let mut wasi = WasiCtxBuilder::new();
        wasi.allow_tcp(false).allow_udp(false).allow_ip_name_lookup(false);
        let state = State {
            wasi: wasi.build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new().memory_size(MEMORY).memories(8).tables(64).table_elements(1 << 20).instances(64).build(),
            http: self.http.clone(),
            storage: self.storage.clone(),
            plugin: plugin.into(),
            keys: keys.map(str::to_string),
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_async_yield_and_update(1);
        let guest = self.pre.instantiate_async(&mut store).await?;
        Ok((store, guest))
    }
}

/// Waits for a guest call, instantiation included, at most `limit`: guest code yields at every clock
/// tick, so even a plugin stuck in a loop is dropped.
async fn finish<R, T>(limit: Duration, call: impl Future<Output = wasmtime::Result<R>>, map: impl FnOnce(R) -> Result<std::result::Result<T, wit::Error>>) -> Result<T> {
    let returned = tokio::time::timeout(limit, call)
        .await
        .map_err(|_| IptvError::Plugin("took too long".into()))?
        .map_err(plugin_failed)?;
    map(returned)?.map_err(convert::from_wit_error)
}

fn plugin_failed(e: wasmtime::Error) -> IptvError {
    IptvError::Plugin(format!("{e:#}"))
}

struct WasmProvider {
    info: ProviderInfo,
    runner: Runner,
}

impl WasmProvider {
    async fn call<R, T>(&self, run: impl AsyncFnOnce(&mut Store<State>, bindings::Provider) -> wasmtime::Result<std::result::Result<R, wit::Error>>, map: impl FnOnce(R) -> Result<T>) -> Result<T> {
        self.call_within(CALL_TIMEOUT, run, map).await
    }

    async fn call_within<R, T>(
        &self,
        limit: Duration,
        run: impl AsyncFnOnce(&mut Store<State>, bindings::Provider) -> wasmtime::Result<std::result::Result<R, wit::Error>>,
        map: impl FnOnce(R) -> Result<T>,
    ) -> Result<T> {
        let call = async {
            let (mut store, guest) = self.runner.instance(&self.info.id, Some(&self.info.id)).await?;
            run(&mut store, guest).await
        };
        map(finish(limit, call, Ok).await?)
    }
}

impl Provider for WasmProvider {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn catalog<'a>(&'a self, config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
        Box::pin(async move {
            self.call(async |store, guest| guest.call_load_catalog(store, &convert::config(config)).await, |c| Ok(convert::catalog(c, self.info.resolves_streams))).await
        })
    }

    fn stream_url<'a>(&'a self, config: &'a Config, channel: &'a Channel) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async move {
            if !self.info.resolves_streams {
                return Ok(channel.stream.clone().into());
            }
            self.call(async |store, guest| guest.call_stream_url(store, &convert::config(config), &convert::to_wit_channel(channel)).await, convert::playback).await
        })
    }

    fn catchup_url<'a>(&'a self, config: &'a Config, channel: &'a Channel, start: i64, end: i64) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async move {
            self.call(async |store, guest| guest.call_catchup_url(store, &convert::config(config), &convert::to_wit_channel(channel), start, end).await, convert::playback).await
        })
    }

    fn episodes<'a>(&'a self, config: &'a Config, series: &'a Series) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            self.call(async |store, guest| guest.call_episodes(store, &convert::config(config), &convert::to_wit_series(series)).await, |e| Ok(e.into_iter().filter_map(|e| convert::episode(e, self.info.resolves_streams)).collect())).await
        })
    }

    fn movie_url<'a>(&'a self, config: &'a Config, movie: &'a Movie) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async move {
            if !self.info.resolves_streams {
                return Playback::stored(&movie.stream);
            }
            self.call(async |store, guest| guest.call_movie_url(store, &convert::config(config), &convert::to_wit_movie(movie)).await, convert::playback).await
        })
    }

    fn episode_url<'a>(&'a self, config: &'a Config, series: &'a Series, episode: &'a Episode) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async move {
            if !self.info.resolves_streams {
                return Playback::stored(&episode.stream);
            }
            let (series, episode) = (convert::to_wit_series(series), convert::to_wit_episode(episode));
            self.call(async |store, guest| guest.call_episode_url(store, &convert::config(config), &series, &episode).await, convert::playback).await
        })
    }

    fn filter<'a>(&'a self, config: &'a Config, url: &'a Url, media: Media) -> BoxFuture<'a, Result<Media>> {
        Box::pin(async move {
            let media = convert::to_wit_media(media);
            self.call_within(FILTER_TIMEOUT, async |store, guest| guest.call_filter(store, &convert::config(config), url.as_str(), &media).await, |m| Ok(convert::media(m))).await
        })
    }

    fn license<'a>(&'a self, config: &'a Config, request: &'a LicenseRequest) -> BoxFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let request = convert::to_wit_license_request(request);
            self.call_within(LICENSE_TIMEOUT, async |store, guest| guest.call_license(store, &convert::config(config), &request).await, Ok).await
        })
    }
}

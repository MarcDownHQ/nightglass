//! Providers compiled to WebAssembly components against `wit/provider.wit`. Every call runs in a
//! fresh instance with no files, sockets or environment: HTTP goes through `Http::send`.

mod convert;

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use nightglass_core::{BoxFuture, Catalog, Channel, Config, Episode, Http, IptvError, Provider, ProviderInfo, Providers, Result, Series};
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

use bindings::nightglass::provider::{http as wit_http, log as wit_log, types as wit};

const MEMORY: usize = 256 << 20;
const CALL_TIMEOUT: Duration = Duration::from_secs(300);
const EPOCH: Duration = Duration::from_millis(10);

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
    http: Http,
    plugin: String,
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
        let response = self.http.send(&request.method, url, &request.headers, request.body).await.map_err(convert::to_wit_error)?;
        Ok(wit_http::Response { status: response.status, headers: response.headers, body: response.body })
    }
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
        Ok(Host { engine, linker, http: http.clone() })
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
        let runner = Runner { pre, engine: self.engine.clone(), http: self.http.clone() };
        let file = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let (mut store, guest) = runner.instance(&file).await.map_err(|e| e.to_string())?;
        let info = finish(guest.call_describe(&mut store), |info| Ok(Ok(info))).await.map_err(|e| e.to_string())?;
        Ok(Arc::new(WasmProvider { info: convert::info(info)?, runner }))
    }
}

struct Runner {
    pre: bindings::ProviderPre<State>,
    engine: Engine,
    http: Http,
}

impl Runner {
    async fn instance(&self, plugin: &str) -> Result<(Store<State>, bindings::Provider)> {
        let mut wasi = WasiCtxBuilder::new();
        wasi.allow_tcp(false).allow_udp(false).allow_ip_name_lookup(false);
        let state = State {
            wasi: wasi.build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new().memory_size(MEMORY).build(),
            http: self.http.clone(),
            plugin: plugin.into(),
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_async_yield_and_update(1);
        let guest = self.pre.instantiate_async(&mut store).await.map_err(plugin_failed)?;
        Ok((store, guest))
    }
}

/// Waits for a guest call, at most `CALL_TIMEOUT`: guest code yields at every clock tick, so even a
/// plugin stuck in a loop is dropped.
async fn finish<R, T>(call: impl Future<Output = wasmtime::Result<R>>, map: impl FnOnce(R) -> Result<std::result::Result<T, wit::Error>>) -> Result<T> {
    let returned = tokio::time::timeout(CALL_TIMEOUT, call)
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
    async fn instance(&self) -> Result<(Store<State>, bindings::Provider)> {
        self.runner.instance(&self.info.id).await
    }
}

impl Provider for WasmProvider {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn catalog<'a>(&'a self, config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
        Box::pin(async move {
            let (mut store, guest) = self.instance().await?;
            let catalog = finish(guest.call_load_catalog(&mut store, &convert::config(config)), Ok).await?;
            Ok(convert::catalog(catalog))
        })
    }

    fn stream_url<'a>(&'a self, config: &'a Config, channel: &'a Channel) -> BoxFuture<'a, Result<Url>> {
        Box::pin(async move {
            if !self.info.resolves_streams {
                return Ok(channel.stream.clone());
            }
            let (mut store, guest) = self.instance().await?;
            let url = finish(guest.call_stream_url(&mut store, &convert::config(config), &convert::to_wit_channel(channel)), Ok).await?;
            convert::url(&url)
        })
    }

    fn catchup_url<'a>(&'a self, config: &'a Config, channel: &'a Channel, start: i64, end: i64) -> BoxFuture<'a, Result<Url>> {
        Box::pin(async move {
            let (mut store, guest) = self.instance().await?;
            let url = finish(guest.call_catchup_url(&mut store, &convert::config(config), &convert::to_wit_channel(channel), start, end), Ok).await?;
            convert::url(&url)
        })
    }

    fn episodes<'a>(&'a self, config: &'a Config, series: &'a Series) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            let (mut store, guest) = self.instance().await?;
            let episodes = finish(guest.call_episodes(&mut store, &convert::config(config), &convert::to_wit_series(series)), Ok).await?;
            Ok(episodes.into_iter().filter_map(convert::episode).collect())
        })
    }
}

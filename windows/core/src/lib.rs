//! IPTV protocols without UI. Each client turns its protocol into the common models
//! (`Channel`, `Movie`, `Series`, `Programme`): the app never knows where a channel came from.

mod builtin;
mod decrypt;
mod http;
pub mod m3u;
mod models;
mod provider;
mod relay;
mod stalker;
pub mod widevine;
pub mod xmltv;
mod xtream;

pub use decrypt::{Decryptor, Unavailable};
pub use http::{BodyReader, Browser, Http, OnDisk, Opened, Request, Response};
pub use m3u::M3uSource;
pub use models::*;
pub use provider::{BoxFuture, Config, ConfigExt, Field, FieldKind, Preset, Provider, ProviderInfo, Providers};
pub use relay::RelayServer;
pub use stalker::StalkerClient;
pub use xtream::XtreamClient;

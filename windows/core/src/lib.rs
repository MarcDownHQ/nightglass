//! IPTV protocols without UI. Each client turns its protocol into the common models
//! (`Channel`, `Movie`, `Series`, `Programme`): the app never knows where a channel came from.

mod http;
pub mod m3u;
mod models;
mod stalker;
pub mod xmltv;
mod xtream;

pub use http::{Http, OnDisk};
pub use m3u::M3uSource;
pub use models::*;
pub use stalker::StalkerClient;
pub use xtream::XtreamClient;

//! Where a protected playback meets the CDM: the app hands it over once the user has allowed protected content and the
//! CDM is installed, and plays what comes back.

use std::sync::Arc;

use crate::widevine::Cdm;
use crate::{BoxFuture, Config, IptvError, Playback, Provider, Result};

pub trait Decryptor: Send + Sync {
    /// What the player opens in place of `playback`, whose `drm` is set: a stream relayed on 127.0.0.1 and decrypted on
    /// its way, its licenses asked of `provider.license` with `config`, its keys held by `cdm`.
    fn open<'a>(&'a self, cdm: &'a Cdm, provider: Arc<dyn Provider>, config: Config, playback: Playback) -> BoxFuture<'a, Result<Playback>>;
}

/// Until the CDM host exists.
pub struct Unavailable;

impl Decryptor for Unavailable {
    fn open<'a>(&'a self, _cdm: &'a Cdm, _provider: Arc<dyn Provider>, _config: Config, _playback: Playback) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async { Err(IptvError::Drm("not implemented yet".into())) })
    }
}

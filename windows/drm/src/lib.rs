//! Widevine-protected fMP4 made clear for a player that knows nothing of DRM.
//!
//! The CDM runs in a helper process (`nightglass-cdm`) that speaks `protocol`; `client` starts and drives it.
//! Nothing that crosses that boundary depends on the OS or on which CDM build answers.

pub mod cenc;
#[cfg(feature = "client")]
pub mod client;
pub mod mp4;
pub mod protocol;
pub mod pssh;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

/// Why protected media can't be played.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DrmError {
    #[error("This service requires hardware-secured decoding, which Nightglass can't offer.")]
    SecureDecodeRequired { kid: String },
    #[error("Widevine only decrypts this video inside its own decoder, which Nightglass can't use.")]
    DecoderOnly { kid: String },
    #[error("The licence holds no key Nightglass may use ({0}).")]
    NoUsableKey(String),
    #[error("No key for {kid}.")]
    NoKey { kid: String },
    #[error("The licence request failed: {0}")]
    License(String),
    #[error("Widevine refused: {0}")]
    Rejected(String),
    #[error("Widevine failed: {0}")]
    Cdm(String),
    #[error("The Widevine helper stopped: {0}")]
    Helper(String),
    #[error("Protected stream not supported: {0}.")]
    Unsupported(String),
    #[error("Unreadable protected media: {0}.")]
    Malformed(String),
}

pub type Result<T> = std::result::Result<T, DrmError>;

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// The licence round trip, done by whoever knows the service: a challenge and what it asks for in, the server's
/// answer out.
pub type Exchange = std::sync::Arc<dyn Fn(protocol::MessageKind, Vec<u8>) -> BoxFuture<'static, Result<Vec<u8>>> + Send + Sync>;

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim().trim_start_matches("0x").trim_start_matches("0X");
    let text: String = text.chars().filter(|c| *c != '-').collect();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_both_ways() {
        assert_eq!(hex(&[0, 0xab, 0x10]), "00ab10");
        assert_eq!(unhex("0x00AB10"), Some(vec![0, 0xab, 0x10]));
        assert_eq!(unhex("edef8ba9-79d6-4ace"), Some(vec![0xed, 0xef, 0x8b, 0xa9, 0x79, 0xd6, 0x4a, 0xce]));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }
}

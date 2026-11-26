//! What the app and the CDM helper say to each other, over the helper's stdin and stdout.
//!
//! Every frame is two little-endian `u32` lengths, then a JSON header of that first length, then a binary payload
//! of the second: challenges, licences and media travel as bytes, never inside the JSON.
//!
//! The helper speaks first, `Ready`. Then each `Envelope` the app sends gets one `Reply` or `Failed` with its `id`;
//! events about sessions (`SessionMessage`, `Keys`, `Expiration`, `Closed`) come whenever the CDM has them, always
//! before the reply to the request that caused them.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

pub const PROTOCOL: u32 = 1;

/// A frame larger than this is refused: a segment's samples fit many times over.
pub const FRAME_LIMIT: usize = 256 << 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub id: u64,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Request {
    /// Answered `Initialized`.
    #[serde(rename_all = "camelCase")]
    Initialize { distinctive_identifier: bool, persistent_state: bool },
    /// Payload: the licence server's certificate. Answered `Done`.
    SetServerCertificate,
    /// Payload: one or more whole `pssh` boxes. Answered `Session`, then the session's first `SessionMessage`.
    CreateSession,
    /// Payload: the licence server's answer to a `SessionMessage`. Answered `Done`, after the `Keys` it brings.
    UpdateSession { session: String },
    CloseSession { session: String },
    /// Payload: the samples back to back, in `samples` order. Answered `Decrypted`, whose payload is the same
    /// samples clear, at the same sizes.
    Decrypt { samples: Vec<Sample> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Message {
    #[serde(rename_all = "camelCase")]
    Ready { protocol: u32, cdm_version: String },
    Reply { id: u64, reply: Reply },
    Failed { id: u64, failure: Failure },
    /// Payload: what the session needs sent to the licence server.
    SessionMessage { session: String, kind: MessageKind },
    Keys { session: String, keys: Vec<Key> },
    Expiration { session: String, time: f64 },
    Closed { session: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Reply {
    Initialized,
    Session { session: String },
    Done,
    Decrypted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageKind {
    LicenseRequest,
    LicenseRenewal,
    LicenseRelease,
    IndividualizationRequest,
}

impl MessageKind {
    pub fn from_cdm(kind: u32) -> Self {
        match kind {
            1 => MessageKind::LicenseRenewal,
            2 => MessageKind::LicenseRelease,
            3 => MessageKind::IndividualizationRequest,
            _ => MessageKind::LicenseRequest,
        }
    }
}

/// A key the licence named, with the CDM's `cdm::KeyStatus`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    #[serde(with = "hex_bytes")]
    pub kid: Vec<u8>,
    pub status: u32,
    #[serde(default)]
    pub system_code: u32,
}

impl Key {
    pub const USABLE: u32 = 0;
    pub const PENDING: u32 = 5;

    pub fn usable(&self) -> bool {
        self.status == Self::USABLE
    }

    pub fn status_name(&self) -> &'static str {
        match self.status {
            0 => "usable",
            1 => "internal error",
            2 => "expired",
            3 => "output restricted",
            4 => "output downscaled",
            5 => "pending",
            6 => "released",
            _ => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
    /// The CDM's own code, when it rejected a promise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_code: Option<u32>,
    /// The sample of a `Decrypt` that failed, and its key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "hex_option")]
    pub kid: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    /// The CDM rejected a promise.
    Rejected,
    /// `Initialize` reported failure.
    Initialization,
    NoKey,
    /// `Decrypt` refused a key it holds: the licence asks for the CDM's own decoder.
    DecryptError,
    /// The request made no sense (unknown session, payload of the wrong size…).
    Malformed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    /// AES-128-CTR.
    Cenc,
    /// AES-128-CBC with a pattern, the IV restarting with each subsample.
    Cbcs,
}

/// One protected sample of a `Decrypt`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub size: u32,
    pub scheme: Scheme,
    #[serde(with = "hex_bytes")]
    pub kid: Vec<u8>,
    #[serde(with = "hex_bytes")]
    pub iv: Vec<u8>,
    /// Encrypted and skipped 16-byte blocks, for `cbcs`. None means every block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<(u8, u8)>,
    /// Clear bytes then protected bytes, in turn. None means the whole sample is protected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subsamples: Vec<(u32, u32)>,
}

pub fn write_frame(out: &mut impl Write, header: &impl Serialize, payload: &[u8]) -> io::Result<()> {
    let header = serde_json::to_vec(header).map_err(io::Error::other)?;
    out.write_all(&frame_lengths(header.len(), payload.len())?)?;
    out.write_all(&header)?;
    out.write_all(payload)?;
    out.flush()
}

pub fn frame_lengths(header: usize, payload: usize) -> io::Result<[u8; 8]> {
    if header + payload > FRAME_LIMIT {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut lengths = [0; 8];
    lengths[..4].copy_from_slice(&(header as u32).to_le_bytes());
    lengths[4..].copy_from_slice(&(payload as u32).to_le_bytes());
    Ok(lengths)
}

/// The lengths that start a frame, checked.
pub fn read_lengths(lengths: [u8; 8]) -> io::Result<(usize, usize)> {
    let header = u32::from_le_bytes(lengths[..4].try_into().expect("4 bytes")) as usize;
    let payload = u32::from_le_bytes(lengths[4..].try_into().expect("4 bytes")) as usize;
    if header + payload > FRAME_LIMIT {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    Ok((header, payload))
}

/// The next frame, none once the stream has ended between frames.
pub fn read_frame<T: for<'de> Deserialize<'de>>(input: &mut impl Read) -> io::Result<Option<(T, Vec<u8>)>> {
    let mut lengths = [0; 8];
    match input.read_exact(&mut lengths) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let (header, payload) = read_lengths(lengths)?;
    let mut header_bytes = vec![0; header];
    input.read_exact(&mut header_bytes)?;
    let mut payload_bytes = vec![0; payload];
    input.read_exact(&mut payload_bytes)?;
    let header = serde_json::from_slice(&header_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some((header, payload_bytes)))
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&crate::hex(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        crate::unhex(&text).ok_or_else(|| serde::de::Error::custom("not hexadecimal"))
    }
}

mod hex_option {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => s.serialize_some(&crate::hex(bytes)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let text = Option::<String>::deserialize(d)?;
        text.map(|t| crate::unhex(&t).ok_or_else(|| serde::de::Error::custom("not hexadecimal"))).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let request = Envelope {
            id: 7,
            request: Request::Decrypt {
                samples: vec![Sample { size: 32, scheme: Scheme::Cbcs, kid: vec![1; 16], iv: vec![2; 16], pattern: Some((1, 9)), subsamples: vec![(5, 27)] }],
            },
        };
        let mut wire = Vec::new();
        write_frame(&mut wire, &request, &[9; 32]).unwrap();
        write_frame(&mut wire, &Message::Ready { protocol: PROTOCOL, cdm_version: "4.10".into() }, &[]).unwrap();
        let mut input = &wire[..];
        let (back, payload) = read_frame::<Envelope>(&mut input).unwrap().unwrap();
        assert_eq!((back, payload), (request, vec![9; 32]));
        let (ready, _) = read_frame::<Message>(&mut input).unwrap().unwrap();
        assert_eq!(ready, Message::Ready { protocol: PROTOCOL, cdm_version: "4.10".into() });
        assert!(read_frame::<Message>(&mut input).unwrap().is_none(), "the end between frames is not an error");
    }

    #[test]
    fn headers_read_as_documented() {
        let header = serde_json::to_value(Envelope { id: 1, request: Request::UpdateSession { session: "s".into() } }).unwrap();
        assert_eq!(header, serde_json::json!({"id": 1, "op": "updateSession", "session": "s"}));
        let keys: Message = serde_json::from_value(serde_json::json!({"type": "keys", "session": "s", "keys": [{"kid": "00ff", "status": 3}]})).unwrap();
        assert_eq!(keys, Message::Keys { session: "s".into(), keys: vec![Key { kid: vec![0, 0xff], status: 3, system_code: 0 }] });
        let failed = serde_json::to_value(Message::Failed { id: 2, failure: Failure { kind: FailureKind::NoKey, message: "m".into(), system_code: None, sample: Some(4), kid: Some(vec![0xab]) } }).unwrap();
        assert_eq!(failed, serde_json::json!({"type": "failed", "id": 2, "failure": {"kind": "noKey", "message": "m", "sample": 4, "kid": "ab"}}));
    }

    #[test]
    fn oversized_and_cut_frames_are_errors() {
        let mut lengths = Vec::new();
        lengths.extend_from_slice(&(FRAME_LIMIT as u32).to_le_bytes());
        lengths.extend_from_slice(&1u32.to_le_bytes());
        assert!(read_frame::<Message>(&mut &lengths[..]).is_err());
        let mut wire = Vec::new();
        write_frame(&mut wire, &Reply::Done, &[1, 2, 3]).unwrap();
        wire.pop();
        assert!(read_frame::<Reply>(&mut &wire[..]).is_err());
    }
}

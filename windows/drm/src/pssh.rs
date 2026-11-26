//! `pssh` boxes (ISO/IEC 23001-7 §8.1): what a licence is asked for with.

use base64::Engine;

use crate::{DrmError, Result};

pub const WIDEVINE: [u8; 16] = [0xed, 0xef, 0x8b, 0xa9, 0x79, 0xd6, 0x4a, 0xce, 0xa3, 0xc8, 0x27, 0xdc, 0xd5, 0x1d, 0x21, 0xed];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pssh {
    pub system: [u8; 16],
    /// Version 1 boxes list their key IDs outside the system's data.
    pub kids: Vec<[u8; 16]>,
    pub data: Vec<u8>,
    /// The whole box, header included, as the CDM takes it.
    pub bytes: Vec<u8>,
}

impl Pssh {
    pub fn parse(bytes: &[u8]) -> Result<Pssh> {
        let bad = || DrmError::Malformed("pssh box".into());
        let size = u32::from_be_bytes(bytes.get(..4).ok_or_else(bad)?.try_into().expect("4 bytes")) as usize;
        if size != bytes.len() || bytes.get(4..8) != Some(b"pssh") {
            return Err(bad());
        }
        let version = *bytes.get(8).ok_or_else(bad)?;
        let system: [u8; 16] = bytes.get(12..28).ok_or_else(bad)?.try_into().expect("16 bytes");
        let mut at = 28;
        let mut kids = Vec::new();
        if version > 0 {
            let count = u32::from_be_bytes(bytes.get(at..at + 4).ok_or_else(bad)?.try_into().expect("4 bytes")) as usize;
            at += 4;
            for _ in 0..count {
                kids.push(bytes.get(at..at + 16).ok_or_else(bad)?.try_into().expect("16 bytes"));
                at += 16;
            }
        }
        let length = u32::from_be_bytes(bytes.get(at..at + 4).ok_or_else(bad)?.try_into().expect("4 bytes")) as usize;
        let data = bytes.get(at + 4..at + 4 + length).ok_or_else(bad)?.to_vec();
        Ok(Pssh { system, kids, data, bytes: bytes.to_vec() })
    }

    /// A version 0 box around a system's data.
    pub fn new(system: [u8; 16], data: &[u8]) -> Pssh {
        let mut bytes = Vec::with_capacity(32 + data.len());
        bytes.extend_from_slice(&(32 + data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(b"pssh");
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&system);
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
        Pssh { system, kids: Vec::new(), data: data.to_vec(), bytes }
    }

    pub fn is_widevine(&self) -> bool {
        self.system == WIDEVINE
    }

    /// The key IDs it names: a version 1 box's own list, or `key_ids` (field 2) of Widevine's data.
    pub fn kids(&self) -> Vec<[u8; 16]> {
        if !self.kids.is_empty() || !self.is_widevine() {
            return self.kids.clone();
        }
        protobuf_fields(&self.data, 2).into_iter().filter_map(|f| f.try_into().ok()).collect()
    }

    /// Base64 as manifests carry it: a whole box (DASH `cenc:pssh`, most HLS `data:` URIs), or only a system's
    /// data, which some HLS packagers put in the `data:` URI.
    pub fn from_base64(text: &str, system: [u8; 16]) -> Result<Pssh> {
        let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(text.trim_end_matches('='))
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(text.trim_end_matches('=')))
            .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(text.trim_end_matches('=')))
            .map_err(|_| DrmError::Malformed("pssh is not base64".into()))?;
        if bytes.get(4..8) == Some(b"pssh") {
            return Pssh::parse(&bytes);
        }
        Ok(Pssh::new(system, &bytes))
    }

    /// Every `pssh` box among `boxes`, laid one after the other.
    pub fn all(mut boxes: &[u8]) -> Vec<Pssh> {
        let mut found = Vec::new();
        while boxes.len() >= 8 {
            let size = u32::from_be_bytes(boxes[..4].try_into().expect("4 bytes")) as usize;
            if size < 8 || size > boxes.len() {
                break;
            }
            if let Ok(pssh) = Pssh::parse(&boxes[..size]) {
                found.push(pssh);
            }
            boxes = &boxes[size..];
        }
        found
    }
}

/// The length-delimited values of `field` at the top level of a protobuf message.
fn protobuf_fields(mut message: &[u8], field: u64) -> Vec<&[u8]> {
    let mut found = Vec::new();
    while let Some((tag, rest)) = varint(message) {
        message = rest;
        let value = match tag & 7 {
            0 => varint(message).map(|(_, rest)| (None, rest)),
            1 => message.get(8..).map(|rest| (None, rest)),
            2 => varint(message).and_then(|(length, rest)| {
                let length = usize::try_from(length).ok()?;
                Some((Some(rest.get(..length)?), rest.get(length..)?))
            }),
            5 => message.get(4..).map(|rest| (None, rest)),
            _ => None,
        };
        let Some((value, rest)) = value else { break };
        if let Some(value) = value.filter(|_| tag >> 3 == field) {
            found.push(value);
        }
        message = rest;
    }
    found
}

fn varint(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let mut value = 0u64;
    for (i, byte) in bytes.iter().enumerate().take(10) {
        value |= u64::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((value, &bytes[i + 1..]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widevine_data(kids: &[[u8; 16]]) -> Vec<u8> {
        let mut data = vec![0x08, 0x01];
        for kid in kids {
            data.extend_from_slice(&[0x12, 16]);
            data.extend_from_slice(kid);
        }
        data.extend_from_slice(&[0x22, 3]);
        data.extend_from_slice(b"abc");
        data
    }

    #[test]
    fn widevine_boxes_name_their_keys() {
        let pssh = Pssh::new(WIDEVINE, &widevine_data(&[[1; 16], [2; 16]]));
        let back = Pssh::parse(&pssh.bytes).unwrap();
        assert_eq!(back, pssh);
        assert!(back.is_widevine());
        assert_eq!(back.kids(), vec![[1; 16], [2; 16]]);
    }

    #[test]
    fn version_1_boxes_list_their_keys() {
        let mut bytes = vec![0, 0, 0, 0];
        bytes.extend_from_slice(b"pssh");
        bytes.extend_from_slice(&[1, 0, 0, 0]);
        bytes.extend_from_slice(&[7; 16]);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&[9; 16]);
        bytes.extend_from_slice(&0u32.to_be_bytes());
        let size = bytes.len() as u32;
        bytes[..4].copy_from_slice(&size.to_be_bytes());
        let pssh = Pssh::parse(&bytes).unwrap();
        assert_eq!((pssh.system, pssh.kids(), pssh.data.len()), ([7; 16], vec![[9; 16]], 0));
    }

    #[test]
    fn base64_holds_a_box_or_only_its_data() {
        let pssh = Pssh::new(WIDEVINE, &widevine_data(&[[3; 16]]));
        let engine = base64::engine::general_purpose::STANDARD;
        assert_eq!(Pssh::from_base64(&engine.encode(&pssh.bytes), WIDEVINE).unwrap(), pssh);
        assert_eq!(Pssh::from_base64(&engine.encode(&pssh.data), WIDEVINE).unwrap(), pssh, "bare data gets its box");
        assert!(Pssh::from_base64("%%%", WIDEVINE).is_err());
    }

    #[test]
    fn boxes_in_a_row_are_all_found() {
        let a = Pssh::new(WIDEVINE, b"a");
        let b = Pssh::new([5; 16], b"b");
        let mut row = a.bytes.clone();
        row.extend_from_slice(&b.bytes);
        assert_eq!(Pssh::all(&row), vec![a, b]);
        assert!(Pssh::parse(b"\0\0\0\x08free").is_err());
    }
}

//! Protected segments made clear: each encrypted sample decrypted in place, the protection boxes made `free`.

use std::collections::HashMap;
use std::sync::Mutex;

use aes::Aes128;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use base64::Engine;

use crate::mp4::{Init, Segment, clear_init, neutralise};
use crate::protocol::{Key, MessageKind, Sample, Scheme};
use crate::pssh::Pssh;
use crate::{BoxFuture, DrmError, Exchange, Result, hex};

/// What turns encrypted samples clear, and gets the keys for it.
pub trait Cdm: Send + Sync {
    /// `data` holds `samples` back to back; they come back clear, at the same sizes.
    fn decrypt(&self, samples: Vec<Sample>, data: Vec<u8>) -> BoxFuture<'_, Result<Vec<u8>>>;

    /// A licence for `pssh` through `exchange`: the keys it brought.
    fn license(&self, pssh: Pssh, exchange: Exchange) -> BoxFuture<'_, Result<Vec<Key>>>;

    /// Whether `kid` can be decrypted outside the CDM's own decoder, for a video sample (several subsamples) or for
    /// any other.
    fn probe<'a>(&'a self, kid: &'a [u8], video: bool) -> BoxFuture<'a, Result<()>>;

    /// Every key the licences so far named, with its status.
    fn keys(&self) -> Vec<Key>;
}

/// The init segment with its sample entries back to their own format and its protection boxes `free`.
pub fn clear_init_segment(data: &mut [u8]) -> Result<Init> {
    let init = Init::parse(data)?;
    clear_init(data, &init);
    Ok(init)
}

/// Every encrypted sample of `data`, a media segment of `init`'s tracks, decrypted in place, and its protection
/// boxes `free`.
pub async fn clear_segment(data: &mut [u8], segment: &Segment, cdm: &dyn Cdm) -> Result<()> {
    if !segment.encrypted.is_empty() {
        let samples: Vec<Sample> = segment.encrypted.iter().map(|e| e.sample.clone()).collect();
        let mut payload = Vec::with_capacity(samples.iter().map(|s| s.size as usize).sum());
        for e in &segment.encrypted {
            payload.extend_from_slice(&data[e.offset..e.offset + e.sample.size as usize]);
        }
        let clear = cdm.decrypt(samples, payload).await?;
        let mut at = 0;
        for e in &segment.encrypted {
            let size = e.sample.size as usize;
            data[e.offset..e.offset + size].copy_from_slice(&clear[at..at + size]);
            at += size;
        }
    }
    neutralise(data, &segment.neutral);
    Ok(())
}

/// Keys known in the clear, for W3C Clear Key streams and for tests: the CENC schemes in software.
#[derive(Default)]
pub struct ClearKeys(Mutex<HashMap<Vec<u8>, [u8; 16]>>);

impl ClearKeys {
    pub fn new(keys: impl IntoIterator<Item = ([u8; 16], [u8; 16])>) -> Self {
        ClearKeys(Mutex::new(keys.into_iter().map(|(kid, key)| (kid.to_vec(), key)).collect()))
    }

    fn key(&self, kid: &[u8]) -> Result<[u8; 16]> {
        lock(&self.0).get(kid).copied().ok_or_else(|| DrmError::NoKey { kid: hex(kid) })
    }

    /// A Clear Key licence request (W3C Encrypted Media Extensions §9.1.3) for the key IDs `pssh` names.
    pub fn request(pssh: &Pssh) -> Vec<u8> {
        let kids: Vec<String> = pssh.kids().iter().map(|k| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(k)).collect();
        serde_json::to_vec(&serde_json::json!({"kids": kids, "type": "temporary"})).expect("JSON")
    }

    /// The keys of a Clear Key licence (a JSON Web Key set).
    pub fn read_license(license: &[u8]) -> Result<Vec<([u8; 16], [u8; 16])>> {
        let bad = || DrmError::License("not a Clear Key licence".into());
        let set: serde_json::Value = serde_json::from_slice(license).map_err(|_| bad())?;
        let decode = |v: &serde_json::Value| -> Option<[u8; 16]> {
            let text = v.as_str()?.trim_end_matches('=');
            base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(text).ok()?.try_into().ok()
        };
        set["keys"].as_array().ok_or_else(bad)?.iter().map(|k| decode(&k["kid"]).zip(decode(&k["k"])).ok_or_else(bad)).collect()
    }
}

impl Cdm for ClearKeys {
    fn decrypt(&self, samples: Vec<Sample>, mut data: Vec<u8>) -> BoxFuture<'_, Result<Vec<u8>>> {
        Box::pin(async move {
            let mut at = 0;
            for sample in &samples {
                let end = at + sample.size as usize;
                let part = data.get_mut(at..end).ok_or_else(|| DrmError::Malformed("samples larger than their data".into()))?;
                decrypt_sample(&self.key(&sample.kid)?, sample, part)?;
                at = end;
            }
            Ok(data)
        })
    }

    fn license(&self, pssh: Pssh, exchange: Exchange) -> BoxFuture<'_, Result<Vec<Key>>> {
        Box::pin(async move {
            let license = exchange(MessageKind::LicenseRequest, ClearKeys::request(&pssh)).await?;
            let keys = ClearKeys::read_license(&license)?;
            let mut known = lock(&self.0);
            known.extend(keys.iter().map(|(kid, key)| (kid.to_vec(), *key)));
            Ok(keys.into_iter().map(|(kid, _)| Key { kid: kid.to_vec(), status: Key::USABLE, system_code: 0 }).collect())
        })
    }

    fn probe<'a>(&'a self, kid: &'a [u8], _video: bool) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move { self.key(kid).map(drop) })
    }

    fn keys(&self) -> Vec<Key> {
        lock(&self.0).keys().map(|kid| Key { kid: kid.clone(), status: Key::USABLE, system_code: 0 }).collect()
    }
}

/// The ranges of a sample its subsamples protect, all of it when it has none.
fn protected_ranges(sample: &Sample, size: usize) -> Result<Vec<std::ops::Range<usize>>> {
    if sample.subsamples.is_empty() {
        return Ok(std::iter::once(0..size).collect());
    }
    let mut at = 0usize;
    let mut ranges = Vec::new();
    for &(clear, protected) in &sample.subsamples {
        let start = at + clear as usize;
        at = start + protected as usize;
        if at > size {
            return Err(DrmError::Malformed("subsamples larger than their sample".into()));
        }
        if protected > 0 {
            ranges.push(start..at);
        }
    }
    Ok(ranges)
}

/// One sample, decrypted in place with `key`.
pub fn decrypt_sample(key: &[u8; 16], sample: &Sample, data: &mut [u8]) -> Result<()> {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let iv: [u8; 16] = sample.iv.as_slice().try_into().map_err(|_| DrmError::Malformed(format!("IV of {} bytes", sample.iv.len())))?;
    let ranges = protected_ranges(sample, data.len())?;
    match sample.scheme {
        Scheme::Cenc => ctr(&cipher, iv, data, &ranges),
        Scheme::Cbcs => {
            let pattern = sample.pattern.unwrap_or((0, 0));
            for range in ranges {
                cbcs_decrypt(&cipher, iv, &mut data[range], pattern);
            }
        }
    }
    Ok(())
}

/// One sample, encrypted in place with `key`: what a packager does, for tests.
pub fn encrypt_sample(key: &[u8; 16], sample: &Sample, data: &mut [u8]) -> Result<()> {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let iv: [u8; 16] = sample.iv.as_slice().try_into().map_err(|_| DrmError::Malformed("IV".into()))?;
    let ranges = protected_ranges(sample, data.len())?;
    match sample.scheme {
        Scheme::Cenc => ctr(&cipher, iv, data, &ranges),
        Scheme::Cbcs => {
            for range in ranges {
                cbcs_encrypt(&cipher, iv, &mut data[range], sample.pattern.unwrap_or((0, 0)));
            }
        }
    }
    Ok(())
}

/// AES-CTR over the protected ranges as one stream: the counter goes on from one range to the next.
fn ctr(cipher: &Aes128, iv: [u8; 16], data: &mut [u8], ranges: &[std::ops::Range<usize>]) {
    let mut counter = u128::from_be_bytes(iv);
    let mut stream = [0u8; 16];
    let mut used = 16;
    for range in ranges {
        for byte in &mut data[range.clone()] {
            if used == 16 {
                let mut block = GenericArray::from(counter.to_be_bytes());
                cipher.encrypt_block(&mut block);
                stream.copy_from_slice(&block);
                counter = counter.wrapping_add(1);
                used = 0;
            }
            *byte ^= stream[used];
            used += 1;
        }
    }
}

/// Whether block `index` of a protected range is encrypted under `pattern` (encrypted, skipped).
fn encrypted_block(index: usize, (crypt, skip): (u8, u8)) -> bool {
    if skip == 0 {
        return true;
    }
    index % (crypt as usize + skip as usize) < crypt as usize
}

/// AES-CBC from the start of the range with `iv`, on the blocks `pattern` encrypts; the chain skips the others,
/// and a last block shorter than 16 bytes stays clear.
fn cbcs_decrypt(cipher: &Aes128, iv: [u8; 16], data: &mut [u8], pattern: (u8, u8)) {
    let mut previous = iv;
    for (index, chunk) in data.as_chunks_mut::<16>().0.iter_mut().enumerate() {
        if !encrypted_block(index, pattern) {
            continue;
        }
        let encrypted = *chunk;
        let mut block = GenericArray::from(encrypted);
        cipher.decrypt_block(&mut block);
        for (out, (b, p)) in chunk.iter_mut().zip(block.iter().zip(previous)) {
            *out = b ^ p;
        }
        previous = encrypted;
    }
}

fn cbcs_encrypt(cipher: &Aes128, iv: [u8; 16], data: &mut [u8], pattern: (u8, u8)) {
    let mut previous = iv;
    for (index, chunk) in data.as_chunks_mut::<16>().0.iter_mut().enumerate() {
        if !encrypted_block(index, pattern) {
            continue;
        }
        let mut block = GenericArray::default();
        for (b, (c, p)) in block.iter_mut().zip(chunk.iter().zip(previous)) {
            *b = c ^ p;
        }
        cipher.encrypt_block(&mut block);
        chunk.copy_from_slice(&block);
        previous = block.into();
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::mp4::Crypto;
    use crate::testing::*;

    fn bytes(text: &str) -> Vec<u8> {
        crate::unhex(text).unwrap()
    }

    const KEY: &str = "2b7e151628aed2a6abf7158809cf4f3c";
    const P1: &str = "6bc1bee22e409f96e93d7e117393172a";
    const P2: &str = "ae2d8a571e03ac9c9eb76fac45af8e51";

    fn key() -> [u8; 16] {
        bytes(KEY).try_into().unwrap()
    }

    fn sample(scheme: Scheme, iv: &str, pattern: Option<(u8, u8)>, subsamples: Vec<(u32, u32)>, size: usize) -> Sample {
        Sample { size: size as u32, scheme, kid: vec![1; 16], iv: bytes(iv), pattern, subsamples }
    }

    #[test]
    fn cenc_counts_on_across_subsamples() {
        let cipher = [bytes("874d6191b620e3261bef6864990db6ce"), bytes("9806f66b7970fdff8617187bb9fffdff")].concat();
        let plain = [bytes(P1), bytes(P2)].concat();
        let mut data = [b"abc".to_vec(), cipher[..10].to_vec(), b"hello".to_vec(), cipher[10..].to_vec()].concat();
        let s = sample(Scheme::Cenc, "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff", None, vec![(3, 10), (5, 22)], data.len());
        decrypt_sample(&key(), &s, &mut data).unwrap();
        assert_eq!(data, [b"abc".to_vec(), plain[..10].to_vec(), b"hello".to_vec(), plain[10..].to_vec()].concat(), "NIST SP 800-38A F.5.2");
    }

    #[test]
    fn cbcs_follows_its_pattern_and_restarts_with_each_subsample() {
        let (c1, c2) = (bytes("7649abac8119b246cee98e9b12e9197d"), bytes("5086cb9b507219ee95db113a917678b2"));
        let skipped = vec![7u8; 16];
        let mut data = [c1.clone(), skipped.clone(), c2, b"tail!".to_vec()].concat();
        let s = sample(Scheme::Cbcs, "000102030405060708090a0b0c0d0e0f", Some((1, 1)), vec![], data.len());
        decrypt_sample(&key(), &s, &mut data).unwrap();
        assert_eq!(data, [bytes(P1), skipped, bytes(P2), b"tail!".to_vec()].concat(), "NIST SP 800-38A F.2.2, one block in two");

        let mut data = [b"nal".to_vec(), c1.clone(), b"nal".to_vec(), c1].concat();
        let s = sample(Scheme::Cbcs, "000102030405060708090a0b0c0d0e0f", Some((1, 9)), vec![(3, 16), (3, 16)], data.len());
        decrypt_sample(&key(), &s, &mut data).unwrap();
        assert_eq!(data, [b"nal".to_vec(), bytes(P1), b"nal".to_vec(), bytes(P1)].concat());
    }

    #[test]
    fn encryption_and_decryption_meet() {
        for (scheme, pattern) in [(Scheme::Cenc, None), (Scheme::Cbcs, Some((1, 9))), (Scheme::Cbcs, Some((0, 0)))] {
            let plain: Vec<u8> = (0..700).map(|i| (i * 7) as u8).collect();
            let s = sample(scheme, "00112233445566778899aabbccddeeff", pattern, vec![(5, 300), (10, 0), (2, 383)], plain.len());
            let mut data = plain.clone();
            encrypt_sample(&key(), &s, &mut data).unwrap();
            assert_ne!(data, plain);
            assert_eq!(&data[..5], &plain[..5], "clear bytes stay clear");
            decrypt_sample(&key(), &s, &mut data).unwrap();
            assert_eq!(data, plain, "{scheme:?} {pattern:?}");
        }
    }

    async fn round_trip(spec: TrackSpec, aux: Aux) {
        let key = [0x42; 16];
        let plain: Vec<SampleSpec> = vec![
            SampleSpec { data: (0..100).map(|i| i as u8).collect(), iv: if spec.crypto.iv_size > 0 { vec![1; spec.crypto.iv_size as usize] } else { vec![] }, subsamples: vec![(10, 64), (6, 20)] },
            SampleSpec { data: vec![9; 48], iv: if spec.crypto.iv_size > 0 { vec![2; spec.crypto.iv_size as usize] } else { vec![] }, subsamples: vec![] },
        ];
        let mut init = init_segment(std::slice::from_ref(&spec), &[Pssh::new(crate::pssh::WIDEVINE, b"x")]);
        let mut segment = media_segment(&spec, &encrypt(&spec, &key, &plain), aux, &[]);
        let expected = media_segment(&spec, &plain, aux, &[]);
        let parsed = clear_init_segment(&mut init).unwrap();
        assert!(!find(&init, b"encv") && !find(&init, b"sinf") && !find(&init, b"pssh"));
        let cdm = ClearKeys::new([(spec.crypto.kid, key)]);
        let found = Segment::parse(&segment, &parsed, 0).unwrap();
        clear_segment(&mut segment, &found, &cdm).await.unwrap();
        let mdat = |d: &[u8]| d[d.windows(4).position(|w| w == b"mdat").unwrap()..].to_vec();
        assert_eq!(mdat(&segment), mdat(&expected), "{:?} {aux:?}", spec.scheme);
        assert_eq!(segment.len(), expected.len());
        assert!(!find(&segment, b"senc") && !find(&segment, b"saio"));
    }

    #[tokio::test]
    async fn segments_come_out_clear_in_both_schemes() {
        let cenc = TrackSpec { id: 1, audio: false, scheme: *b"cenc", crypto: crypto([5; 16], 8, None, (0, 0)) };
        let cenc16 = TrackSpec { id: 3, audio: true, scheme: *b"cenc", crypto: crypto([5; 16], 16, None, (0, 0)) };
        let cbcs = TrackSpec { id: 2, audio: false, scheme: *b"cbcs", crypto: crypto([6; 16], 0, Some(vec![3; 16]), (1, 9)) };
        for aux in [Aux::Senc, Aux::SaizSaio] {
            round_trip(cenc.clone(), aux).await;
            round_trip(cenc16.clone(), aux).await;
            round_trip(cbcs.clone(), aux).await;
        }
    }

    #[tokio::test]
    async fn rotated_keys_decrypt_with_their_own() {
        let spec = TrackSpec { id: 1, audio: false, scheme: *b"cbcs", crypto: crypto([5; 16], 0, Some(vec![3; 16]), (1, 9)) };
        let rotated = TrackSpec { crypto: crypto([8; 16], 0, Some(vec![4; 16]), (1, 9)), ..spec.clone() };
        let plain: Vec<SampleSpec> = (0..3).map(|i| SampleSpec { data: vec![i + 1; 64], iv: vec![], subsamples: vec![(16, 48)] }).collect();
        let mut encrypted = encrypt(&spec, &[1; 16], &plain[..1]);
        encrypted.extend(encrypt(&rotated, &[2; 16], &plain[1..2]));
        encrypted.push(plain[2].clone());
        let clear = Crypto { protected: false, iv_size: 0, kid: [0; 16], constant_iv: vec![], pattern: (0, 0) };
        let runs = [(1, 0), (1, 0x10001), (1, 0x10002)];
        let mut segment = media_segment_with_groups(&spec, &encrypted, &[rotated.crypto.clone(), clear.clone()], &runs);
        let expected = media_segment_with_groups(&spec, &plain, &[rotated.crypto.clone(), clear], &runs);
        let init = Init::parse(&init_segment(&[spec], &[])).unwrap();
        let found = Segment::parse(&segment, &init, 0).unwrap();
        let cdm = ClearKeys::new([([5; 16], [1; 16])]);
        assert_eq!(clear_segment(&mut segment.clone(), &found, &cdm).await, Err(DrmError::NoKey { kid: hex(&[8; 16]) }));
        let cdm = ClearKeys::new([([5; 16], [1; 16]), ([8; 16], [2; 16])]);
        clear_segment(&mut segment, &found, &cdm).await.unwrap();
        let mdat = |d: &[u8]| d[d.windows(4).position(|w| w == b"mdat").unwrap()..].to_vec();
        assert_eq!(mdat(&segment), mdat(&expected));
    }

    #[tokio::test]
    async fn clear_key_licences_go_through_the_exchange() {
        let pssh = Pssh::new(crate::pssh::WIDEVINE, &[[0x12, 16].as_slice(), &[7; 16]].concat());
        let exchange: Exchange = Arc::new(|kind, request: Vec<u8>| {
            Box::pin(async move {
                assert_eq!(kind, MessageKind::LicenseRequest);
                let request: serde_json::Value = serde_json::from_slice(&request).unwrap();
                assert_eq!(request["kids"], serde_json::json!(["BwcHBwcHBwcHBwcHBwcHBw"]));
                Ok(br#"{"keys":[{"kty":"oct","kid":"BwcHBwcHBwcHBwcHBwcHBw","k":"CQkJCQkJCQkJCQkJCQkJCQ"}]}"#.to_vec())
            })
        });
        let cdm = ClearKeys::default();
        assert!(cdm.probe(&[7; 16], true).await.is_err());
        let keys = cdm.license(pssh, exchange).await.unwrap();
        assert_eq!(keys, vec![Key { kid: vec![7; 16], status: Key::USABLE, system_code: 0 }]);
        cdm.probe(&[7; 16], true).await.unwrap();
        assert_eq!(cdm.key(&[7; 16]).unwrap(), [9; 16]);
    }
}

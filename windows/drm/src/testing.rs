//! Synthetic protected fMP4 for tests: small init and media segments built box by box.

use crate::mp4::Crypto;
use crate::pssh::Pssh;

pub fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + body.len());
    b.extend_from_slice(&(8 + body.len() as u32).to_be_bytes());
    b.extend_from_slice(kind);
    b.extend_from_slice(body);
    b
}

pub fn full(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut b = ((u32::from(version) << 24) | flags).to_be_bytes().to_vec();
    b.extend_from_slice(body);
    bx(kind, &b)
}

/// Whether `kind` appears anywhere in `data`.
pub fn find(data: &[u8], kind: &[u8; 4]) -> bool {
    data.windows(4).any(|w| w == kind)
}

pub fn crypto(kid: [u8; 16], iv_size: u8, constant_iv: Option<Vec<u8>>, pattern: (u8, u8)) -> Crypto {
    Crypto { protected: true, iv_size, kid, constant_iv: constant_iv.unwrap_or_default(), pattern }
}

#[derive(Clone, Debug)]
pub struct TrackSpec {
    pub id: u32,
    pub audio: bool,
    pub scheme: [u8; 4],
    pub crypto: Crypto,
}

#[derive(Clone, Debug)]
pub struct SampleSpec {
    pub data: Vec<u8>,
    pub iv: Vec<u8>,
    pub subsamples: Vec<(u32, u32)>,
}

#[derive(Clone, Copy, Debug)]
pub enum Aux {
    Senc,
    SaizSaio,
}

/// `tenc` and `seig` fields, after a full box header.
pub fn crypto_fields(c: &Crypto) -> Vec<u8> {
    let mut b = vec![0, (c.pattern.0 << 4) | c.pattern.1, u8::from(c.protected), c.iv_size];
    b.extend_from_slice(&c.kid);
    if c.protected && c.iv_size == 0 {
        b.push(c.constant_iv.len() as u8);
        b.extend_from_slice(&c.constant_iv);
    }
    b
}

fn sample_entry(track: &TrackSpec) -> Vec<u8> {
    let tenc = full(b"tenc", u8::from(track.crypto.pattern != (0, 0)), 0, &crypto_fields(&track.crypto));
    let mut schm = track.scheme.to_vec();
    schm.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    let format = if track.audio { b"mp4a" } else { b"avc1" };
    let sinf = bx(b"sinf", &[bx(b"frma", format), full(b"schm", 0, 0, &schm), bx(b"schi", &tenc)].concat());
    if track.audio {
        bx(b"enca", &[vec![0; 28], bx(b"esds", &[0; 4]), sinf].concat())
    } else {
        bx(b"encv", &[vec![0; 78], bx(b"avcC", &[1, 2, 3]), sinf].concat())
    }
}

pub fn init_segment(tracks: &[TrackSpec], pssh: &[Pssh]) -> Vec<u8> {
    let mut moov = full(b"mvhd", 0, 0, &[0; 96]);
    let mut mvex = Vec::new();
    for track in tracks {
        let mut tkhd = vec![0; 8];
        tkhd.extend_from_slice(&track.id.to_be_bytes());
        tkhd.extend_from_slice(&[0; 68]);
        let stsd = full(b"stsd", 0, 0, &[1u32.to_be_bytes().to_vec(), sample_entry(track)].concat());
        let stbl = bx(b"stbl", &stsd);
        let mdia = bx(b"mdia", &[full(b"mdhd", 0, 0, &[0; 20]), bx(b"minf", &stbl)].concat());
        moov.extend_from_slice(&bx(b"trak", &[full(b"tkhd", 0, 7, &tkhd), mdia].concat()));
        let mut trex = track.id.to_be_bytes().to_vec();
        trex.extend_from_slice(&1u32.to_be_bytes());
        trex.extend_from_slice(&[0; 12]);
        mvex.extend_from_slice(&full(b"trex", 0, 0, &trex));
    }
    moov.extend_from_slice(&bx(b"mvex", &mvex));
    for p in pssh {
        moov.extend_from_slice(&p.bytes);
    }
    [bx(b"ftyp", b"iso6\0\0\0\0iso6cmfc"), bx(b"moov", &moov)].concat()
}

fn aux_info(sample: &SampleSpec) -> Vec<u8> {
    let mut info = sample.iv.clone();
    if !sample.subsamples.is_empty() {
        info.extend_from_slice(&(sample.subsamples.len() as u16).to_be_bytes());
        for &(clear, protected) in &sample.subsamples {
            info.extend_from_slice(&(clear as u16).to_be_bytes());
            info.extend_from_slice(&protected.to_be_bytes());
        }
    }
    info
}

fn senc(samples: &[SampleSpec]) -> Vec<u8> {
    let mut body = (samples.len() as u32).to_be_bytes().to_vec();
    for sample in samples {
        let mut info = aux_info(sample);
        if sample.subsamples.is_empty() {
            info.extend_from_slice(&[0, 0]);
        }
        body.extend_from_slice(&info);
    }
    full(b"senc", 0, 2, &body)
}

fn trun(samples: &[SampleSpec], data_offset: u32) -> Vec<u8> {
    let mut body = (samples.len() as u32).to_be_bytes().to_vec();
    body.extend_from_slice(&data_offset.to_be_bytes());
    for sample in samples {
        body.extend_from_slice(&(sample.data.len() as u32).to_be_bytes());
    }
    full(b"trun", 0, 0x201, &body)
}

fn tfhd(track: u32) -> Vec<u8> {
    full(b"tfhd", 0, 0x20000, &track.to_be_bytes())
}

/// `moof` + `mdat`, the `moof` built by `traf` once its data offset is known.
fn fragment(samples: &[SampleSpec], moof: impl Fn(u32) -> Vec<u8>) -> Vec<u8> {
    let size = moof(0).len() as u32;
    let mdat: Vec<u8> = samples.iter().flat_map(|s| s.data.clone()).collect();
    [moof(size + 8), bx(b"mdat", &mdat)].concat()
}

pub fn media_segment(track: &TrackSpec, samples: &[SampleSpec], aux: Aux, pssh: &[Pssh]) -> Vec<u8> {
    fragment(samples, |offset| {
        let head = [tfhd(track.id), full(b"tfdt", 1, 0, &[0; 8]), trun(samples, offset)].concat();
        let traf = match aux {
            Aux::Senc => [head, senc(samples)].concat(),
            Aux::SaizSaio => {
                let infos: Vec<Vec<u8>> = samples.iter().map(aux_info).collect();
                let mut saiz = vec![0];
                saiz.extend_from_slice(&(samples.len() as u32).to_be_bytes());
                saiz.extend(infos.iter().map(|i| i.len() as u8));
                let saiz = full(b"saiz", 0, 0, &saiz);
                let saio_size = 20;
                let at = 8 + 16 + 8 + head.len() + saiz.len() + saio_size + 8;
                let mut saio = 1u32.to_be_bytes().to_vec();
                saio.extend_from_slice(&(at as u32).to_be_bytes());
                let saio = full(b"saio", 0, 0, &saio);
                [head, saiz, saio, bx(b"free", &infos.concat())].concat()
            }
        };
        let pssh: Vec<u8> = pssh.iter().flat_map(|p| p.bytes.clone()).collect();
        bx(b"moof", &[full(b"mfhd", 0, 0, &[0, 0, 0, 1]), bx(b"traf", &traf), pssh].concat())
    })
}

/// A segment whose samples belong to `seig` groups: `runs` of (sample count, group index), local groups from
/// 0x10001.
pub fn media_segment_with_groups(track: &TrackSpec, samples: &[SampleSpec], groups: &[Crypto], runs: &[(u32, u32)]) -> Vec<u8> {
    fragment(samples, |offset| {
        let mut sbgp = b"seig".to_vec();
        sbgp.extend_from_slice(&(runs.len() as u32).to_be_bytes());
        for &(count, index) in runs {
            sbgp.extend_from_slice(&count.to_be_bytes());
            sbgp.extend_from_slice(&index.to_be_bytes());
        }
        let mut sgpd = b"seig".to_vec();
        sgpd.extend_from_slice(&0u32.to_be_bytes());
        sgpd.extend_from_slice(&(groups.len() as u32).to_be_bytes());
        for group in groups {
            let fields = crypto_fields(group);
            sgpd.extend_from_slice(&(fields.len() as u32).to_be_bytes());
            sgpd.extend_from_slice(&fields);
        }
        let traf = [tfhd(track.id), trun(samples, offset), full(b"sbgp", 0, 0, &sbgp), full(b"sgpd", 1, 0, &sgpd), senc(samples)].concat();
        bx(b"moof", &[full(b"mfhd", 0, 0, &[0, 0, 0, 1]), bx(b"traf", &traf)].concat())
    })
}

/// `samples` as a packager encrypts them for `track`, each with its own IV unless the track has a constant one.
pub fn encrypt(track: &TrackSpec, key: &[u8; 16], samples: &[SampleSpec]) -> Vec<SampleSpec> {
    use crate::protocol::{Sample, Scheme};
    let scheme = if &track.scheme == b"cbcs" { Scheme::Cbcs } else { Scheme::Cenc };
    samples
        .iter()
        .map(|s| {
            let mut iv = if s.iv.is_empty() { track.crypto.constant_iv.clone() } else { s.iv.clone() };
            iv.resize(16, 0);
            let sample = Sample { size: s.data.len() as u32, scheme, kid: track.crypto.kid.to_vec(), iv, pattern: Some(track.crypto.pattern), subsamples: s.subsamples.clone() };
            let mut data = s.data.clone();
            crate::cenc::encrypt_sample(key, &sample, &mut data).expect("encrypted");
            SampleSpec { data, ..s.clone() }
        })
        .collect()
}

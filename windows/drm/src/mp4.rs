//! Protected fMP4 as ISO/IEC 14496-12 and 23001-7 lay it out: where each encrypted sample is, how it is
//! encrypted, and which boxes say so.

use std::ops::Range;

use crate::protocol::{Sample, Scheme};
use crate::pssh::Pssh;
use crate::{DrmError, Result};

const PIFF_SENC: [u8; 16] = [0xa2, 0x39, 0x4f, 0x52, 0x5a, 0x9b, 0x4f, 0x14, 0xa2, 0x44, 0x6c, 0x42, 0x7c, 0x64, 0x8d, 0xf4];
const NEUTRAL: [u8; 4] = *b"free";

/// One box: its type, and where its header, its body and its end are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bx {
    pub kind: [u8; 4],
    pub start: usize,
    pub body: usize,
    pub end: usize,
}

fn malformed(what: impl std::fmt::Display) -> DrmError {
    DrmError::Malformed(what.to_string())
}

/// The boxes laid one after the other in `range`. A tail too short for a box header is ignored.
pub fn children(data: &[u8], range: Range<usize>) -> Result<Vec<Bx>> {
    let mut found = Vec::new();
    let mut at = range.start;
    while range.end.saturating_sub(at) >= 8 {
        let mut r = Reader::new(data, at..range.end);
        let size = r.u32()?;
        let kind: [u8; 4] = r.array()?;
        let size = match size {
            0 => range.end - at,
            1 => usize::try_from(r.u64()?).map_err(|_| malformed("box too large"))?,
            n => n as usize,
        };
        if kind == *b"uuid" {
            r.skip(16)?;
        }
        let end = at.checked_add(size).filter(|end| *end <= range.end && r.at <= *end);
        let Some(end) = end else { return Err(malformed(format!("{} box overruns its parent", String::from_utf8_lossy(&kind)))) };
        found.push(Bx { kind, start: at, body: r.at, end });
        at = end;
    }
    Ok(found)
}

fn child(data: &[u8], parent: &Bx, kind: &[u8; 4]) -> Result<Option<Bx>> {
    Ok(children(data, parent.body..parent.end)?.into_iter().find(|b| &b.kind == kind))
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    end: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], range: Range<usize>) -> Self {
        Reader { data, at: range.start, end: range.end.min(data.len()) }
    }

    fn of(data: &'a [u8], b: &Bx) -> Self {
        Reader::new(data, b.body..b.end)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).filter(|end| *end <= self.end).ok_or_else(|| malformed("box shorter than its fields"))?;
        let bytes = &self.data[self.at..end];
        self.at = end;
        Ok(bytes)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(drop)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    /// A full box's version and flags.
    fn full(&mut self) -> Result<(u8, u32)> {
        let word = self.u32()?;
        Ok(((word >> 24) as u8, word & 0x00ff_ffff))
    }

    fn left(&self) -> usize {
        self.end - self.at
    }
}

/// How samples are encrypted, from `tenc` or a `seig` sample group entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crypto {
    pub protected: bool,
    pub iv_size: u8,
    pub kid: [u8; 16],
    pub constant_iv: Vec<u8>,
    /// Encrypted and skipped blocks, for pattern schemes.
    pub pattern: (u8, u8),
}

impl Crypto {
    /// The fields `tenc` (after its full box header) and `seig` entries share.
    fn read(r: &mut Reader, has_pattern: bool) -> Result<Crypto> {
        r.skip(1)?;
        let pattern = r.u8()?;
        let protected = r.u8()? != 0;
        let iv_size = r.u8()?;
        let kid = r.array()?;
        let mut constant_iv = Vec::new();
        if protected && iv_size == 0 {
            let size = r.u8()? as usize;
            constant_iv = r.take(size)?.to_vec();
        }
        let pattern = if has_pattern { (pattern >> 4, pattern & 0x0f) } else { (0, 0) };
        Ok(Crypto { protected, iv_size, kid, constant_iv, pattern })
    }
}

/// A protected sample entry (`encv`, `enca`) of a track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// What `frma` says the entry was before protection: `avc1`, `mp4a`…
    pub format: [u8; 4],
    /// `schm`: `cenc`, `cbcs`, `cens` or `cbc1`.
    pub scheme: [u8; 4],
    pub crypto: Crypto,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    pub id: u32,
    /// By sample description index, from 1: none for an entry that is not protected.
    pub entries: Vec<Option<Entry>>,
    /// `seig` entries of the track's `sgpd`, by group description index from 1.
    pub groups: Vec<Crypto>,
    /// `trex` defaults.
    pub default_description: u32,
    pub default_size: u32,
}

/// An init segment (`moov`) and what makes it protected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Init {
    pub tracks: Vec<Track>,
    pub pssh: Vec<Pssh>,
    /// Sample entry types to put back as they were, by the offset of their type.
    pub rename: Vec<(usize, [u8; 4])>,
    /// Offsets of the types of the boxes that only describe the protection.
    pub neutral: Vec<usize>,
}

impl Init {
    pub fn parse(data: &[u8]) -> Result<Init> {
        let mut init = Init::default();
        let Some(moov) = children(data, 0..data.len())?.into_iter().find(|b| &b.kind == b"moov") else {
            return Err(malformed("no moov box"));
        };
        let mut trex = Vec::new();
        for b in children(data, moov.body..moov.end)? {
            match &b.kind {
                b"trak" => {
                    let track = init.parse_trak(data, &b)?;
                    init.tracks.push(track);
                }
                b"pssh" => init.pssh_at(data, &b),
                b"mvex" => {
                    for t in children(data, b.body..b.end)?.into_iter().filter(|t| &t.kind == b"trex") {
                        let mut r = Reader::of(data, &t);
                        r.full()?;
                        let (id, description, _duration, size) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
                        trex.push((id, description, size));
                    }
                }
                _ => {}
            }
        }
        for track in &mut init.tracks {
            if let Some(&(_, description, size)) = trex.iter().find(|(id, ..)| *id == track.id) {
                (track.default_description, track.default_size) = (description, size);
            }
        }
        Ok(init)
    }

    pub fn track(&self, id: u32) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn is_protected(&self) -> bool {
        self.tracks.iter().any(|t| t.entries.iter().flatten().any(|e| e.crypto.protected) || t.groups.iter().any(|g| g.protected))
    }

    /// The default key of every protected entry.
    pub fn kids(&self) -> Vec<[u8; 16]> {
        let mut kids: Vec<[u8; 16]> = self.tracks.iter().flat_map(|t| t.entries.iter().flatten()).filter(|e| e.crypto.protected).map(|e| e.crypto.kid).collect();
        kids.dedup();
        kids
    }

    fn pssh_at(&mut self, data: &[u8], b: &Bx) {
        if let Ok(pssh) = Pssh::parse(&data[b.start..b.end]) {
            self.pssh.push(pssh);
        }
        self.neutral.push(b.start + 4);
    }

    fn parse_trak(&mut self, data: &[u8], trak: &Bx) -> Result<Track> {
        let mut track = Track::default();
        for b in children(data, trak.body..trak.end)? {
            match &b.kind {
                b"tkhd" => {
                    let mut r = Reader::of(data, &b);
                    let (version, _) = r.full()?;
                    r.skip(if version == 1 { 16 } else { 8 })?;
                    track.id = r.u32()?;
                }
                b"pssh" => self.pssh_at(data, &b),
                b"mdia" => {
                    let stbl = child(data, &b, b"minf")?.map(|minf| child(data, &minf, b"stbl")).transpose()?.flatten();
                    let Some(stbl) = stbl else { continue };
                    for s in children(data, stbl.body..stbl.end)? {
                        match &s.kind {
                            b"stsd" => track.entries = self.parse_stsd(data, &s)?,
                            b"sgpd" => {
                                if let Some(groups) = seig_groups(data, &s)? {
                                    track.groups = groups;
                                    self.neutral.push(s.start + 4);
                                }
                            }
                            b"sbgp" if is_seig(data, &s) => self.neutral.push(s.start + 4),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(track)
    }

    fn parse_stsd(&mut self, data: &[u8], stsd: &Bx) -> Result<Vec<Option<Entry>>> {
        let mut entries = Vec::new();
        for entry in children(data, (stsd.body + 8).min(stsd.end)..stsd.end)? {
            let fixed = match &entry.kind {
                b"encv" => 78,
                b"enca" => {
                    let mut r = Reader::new(data, entry.body + 8..entry.end);
                    match r.u16()? {
                        0 => 28,
                        1 => 44,
                        2 => 64,
                        v => return Err(malformed(format!("enca version {v}"))),
                    }
                }
                _ => {
                    entries.push(None);
                    continue;
                }
            };
            let Some(sinf) = children(data, (entry.body + fixed).min(entry.end)..entry.end)?.into_iter().find(|b| &b.kind == b"sinf") else {
                return Err(malformed("protected sample entry without sinf"));
            };
            let (mut format, mut scheme, mut crypto) = (None, None, None);
            for b in children(data, sinf.body..sinf.end)? {
                match &b.kind {
                    b"frma" => format = Some(Reader::of(data, &b).array()?),
                    b"schm" => {
                        let mut r = Reader::of(data, &b);
                        r.full()?;
                        scheme = Some(r.array()?);
                    }
                    b"schi" => {
                        if let Some(tenc) = child(data, &b, b"tenc")? {
                            let mut r = Reader::of(data, &tenc);
                            let (version, _) = r.full()?;
                            crypto = Some(Crypto::read(&mut r, version > 0)?);
                        }
                    }
                    _ => {}
                }
            }
            let (Some(format), Some(scheme), Some(crypto)) = (format, scheme, crypto) else {
                return Err(malformed("sinf without frma, schm or tenc"));
            };
            self.rename.push((entry.start + 4, format));
            self.neutral.push(sinf.start + 4);
            entries.push(Some(Entry { format, scheme, crypto }));
        }
        Ok(entries)
    }
}

fn is_seig(data: &[u8], b: &Bx) -> bool {
    data.get(b.body + 4..b.body + 8) == Some(b"seig")
}

/// The entries of an `sgpd` box, when it groups by `seig`.
fn seig_groups(data: &[u8], b: &Bx) -> Result<Option<Vec<Crypto>>> {
    if !is_seig(data, b) {
        return Ok(None);
    }
    let mut r = Reader::of(data, b);
    let (version, _) = r.full()?;
    r.skip(4)?;
    let default_length = if version == 1 { r.u32()? } else { 0 };
    if version >= 2 {
        r.skip(4)?;
    }
    let count = r.u32()?;
    let mut groups = Vec::new();
    for _ in 0..count {
        let length = if version == 1 && default_length == 0 { Some(r.u32()? as usize) } else if version == 1 { Some(default_length as usize) } else { None };
        let start = r.at;
        groups.push(Crypto::read(&mut r, true)?);
        if let Some(length) = length {
            r.at = start;
            r.skip(length)?;
        }
    }
    Ok(Some(groups))
}

/// A protected sample of a media segment, and where it lies in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encrypted {
    pub offset: usize,
    pub sample: Sample,
}

/// What a media segment holds that matters for its decryption.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Segment {
    pub encrypted: Vec<Encrypted>,
    /// `pssh` boxes of its `moof`s: keys that change along the stream.
    pub pssh: Vec<Pssh>,
    pub neutral: Vec<usize>,
}

impl Segment {
    /// `data` is one or more `moof`/`mdat` pairs of the tracks `init` describes. `origin` is where `data` starts
    /// in the resource it came from, for `tfhd` base data offsets, which count from there.
    pub fn parse(data: &[u8], init: &Init, origin: u64) -> Result<Segment> {
        let mut segment = Segment::default();
        for moof in children(data, 0..data.len())?.into_iter().filter(|b| &b.kind == b"moof") {
            let (mut previous_end, mut first) = (moof.start, true);
            for b in children(data, moof.body..moof.end)? {
                match &b.kind {
                    b"traf" => {
                        previous_end = segment.parse_traf(data, init, origin, &moof, &b, first, previous_end)?;
                        first = false;
                    }
                    b"pssh" => {
                        if let Ok(pssh) = Pssh::parse(&data[b.start..b.end]) {
                            segment.pssh.push(pssh);
                        }
                        segment.neutral.push(b.start + 4);
                    }
                    _ => {}
                }
            }
        }
        Ok(segment)
    }

    /// The end of the traf's sample data, where the next traf's data starts unless told otherwise.
    #[allow(clippy::too_many_arguments)]
    fn parse_traf(&mut self, data: &[u8], init: &Init, origin: u64, moof: &Bx, traf: &Bx, first: bool, previous_end: usize) -> Result<usize> {
        let boxes = children(data, traf.body..traf.end)?;
        let Some(tfhd) = boxes.iter().find(|b| &b.kind == b"tfhd") else { return Err(malformed("traf without tfhd")) };
        let mut r = Reader::of(data, tfhd);
        let (_, flags) = r.full()?;
        let track_id = r.u32()?;
        let Some(track) = init.track(track_id) else { return Err(malformed(format!("track {track_id} is not in the init segment"))) };
        let base_offset = if flags & 0x1 != 0 { Some(r.u64()?) } else { None };
        let description = if flags & 0x2 != 0 { r.u32()? } else { track.default_description.max(1) };
        if flags & 0x8 != 0 {
            r.skip(4)?;
        }
        let default_size = if flags & 0x10 != 0 { r.u32()? } else { track.default_size };
        let base = match base_offset {
            Some(offset) => offset.checked_sub(origin).and_then(|o| usize::try_from(o).ok()).ok_or_else(|| malformed("base data offset before the segment"))?,
            None if flags & 0x20000 != 0 || first => moof.start,
            None => previous_end,
        };

        let mut samples: Vec<Range<usize>> = Vec::new();
        let mut next = base;
        for trun in boxes.iter().filter(|b| &b.kind == b"trun") {
            let mut r = Reader::of(data, trun);
            let (_, flags) = r.full()?;
            let count = r.u32()?;
            if flags & 0x1 != 0 {
                next = base.checked_add_signed(r.u32()? as i32 as isize).ok_or_else(|| malformed("data offset before the segment"))?;
            }
            if flags & 0x4 != 0 {
                r.skip(4)?;
            }
            for _ in 0..count {
                if flags & 0x100 != 0 {
                    r.skip(4)?;
                }
                let size = if flags & 0x200 != 0 { r.u32()? } else { default_size } as usize;
                if flags & 0x400 != 0 {
                    r.skip(4)?;
                }
                if flags & 0x800 != 0 {
                    r.skip(4)?;
                }
                let end = next.checked_add(size).filter(|end| *end <= data.len()).ok_or_else(|| malformed("sample beyond the segment"))?;
                samples.push(next..end);
                next = end;
            }
        }

        let entry = description.checked_sub(1).and_then(|i| track.entries.get(i as usize)).and_then(Option::as_ref);
        let mut groups_local = Vec::new();
        let mut group_runs: Vec<(u32, u32)> = Vec::new();
        for b in &boxes {
            match &b.kind {
                b"sgpd" => {
                    if let Some(groups) = seig_groups(data, b)? {
                        groups_local = groups;
                        self.neutral.push(b.start + 4);
                    }
                }
                b"sbgp" if is_seig(data, b) => {
                    let mut r = Reader::of(data, b);
                    let (version, _) = r.full()?;
                    r.skip(if version == 1 { 8 } else { 4 })?;
                    for _ in 0..r.u32()? {
                        group_runs.push((r.u32()?, r.u32()?));
                    }
                    self.neutral.push(b.start + 4);
                }
                _ => {}
            }
        }
        let mut cryptos = Vec::with_capacity(samples.len());
        let mut runs = group_runs.iter().flat_map(|&(count, index)| std::iter::repeat_n(index, count as usize));
        for _ in 0..samples.len() {
            let crypto = match runs.next().unwrap_or(0) {
                0 => entry.map(|e| &e.crypto),
                i if i > 0x10000 => Some(groups_local.get((i - 0x10001) as usize).ok_or_else(|| malformed("seig group missing from the traf"))?),
                i => Some(track.groups.get(i as usize - 1).ok_or_else(|| malformed("seig group missing from the track"))?),
            };
            cryptos.push(crypto.filter(|c| c.protected));
        }

        let aux = boxes.iter().find(|b| &b.kind == b"senc" || (&b.kind == b"uuid" && data.get(b.start + 8..b.start + 24) == Some(&PIFF_SENC[..])));
        for b in boxes.iter().filter(|b| matches!(&b.kind, b"saiz" | b"saio")).chain(aux) {
            self.neutral.push(b.start + 4);
        }
        if cryptos.iter().all(Option::is_none) {
            return Ok(next);
        }
        let Some(entry) = entry else { return Err(malformed("encrypted samples under a clear sample entry")) };
        let scheme = match &entry.scheme {
            b"cenc" => Scheme::Cenc,
            b"cbcs" => Scheme::Cbcs,
            other => return Err(DrmError::Unsupported(format!("{} encryption", String::from_utf8_lossy(other)))),
        };
        let iv_sizes: Vec<usize> = cryptos.iter().map(|c| c.map_or(0, |c| c.iv_size as usize)).collect();
        let infos = match aux {
            Some(senc) => read_senc(data, senc, &iv_sizes)?,
            None => read_saio(data, &boxes, base, &iv_sizes)?,
        };
        for ((range, crypto), (iv, subsamples)) in samples.into_iter().zip(&cryptos).zip(infos) {
            let Some(crypto) = crypto else { continue };
            let mut iv = if iv.is_empty() { crypto.constant_iv.clone() } else { iv };
            if iv.len() == 8 {
                iv.resize(16, 0);
            }
            if iv.len() != 16 {
                return Err(malformed(format!("IV of {} bytes", iv.len())));
            }
            let covered: u64 = subsamples.iter().map(|&(c, p)| u64::from(c) + u64::from(p)).sum();
            if covered > range.len() as u64 {
                return Err(malformed("subsamples larger than their sample"));
            }
            let pattern = (scheme == Scheme::Cbcs).then_some(crypto.pattern);
            let sample = Sample { size: range.len() as u32, scheme, kid: crypto.kid.to_vec(), iv, pattern, subsamples };
            self.encrypted.push(Encrypted { offset: range.start, sample });
        }
        Ok(next)
    }

    /// Every key its samples need.
    pub fn kids(&self) -> Vec<Vec<u8>> {
        let mut kids: Vec<Vec<u8>> = Vec::new();
        for e in &self.encrypted {
            if !kids.contains(&e.sample.kid) {
                kids.push(e.sample.kid.clone());
            }
        }
        kids
    }
}

type AuxInfo = (Vec<u8>, Vec<(u32, u32)>);

fn read_senc(data: &[u8], senc: &Bx, iv_sizes: &[usize]) -> Result<Vec<AuxInfo>> {
    let mut r = Reader::of(data, senc);
    let (_, flags) = r.full()?;
    let mut override_size = None;
    if flags & 0x1 != 0 {
        r.skip(3)?;
        override_size = Some(r.u8()? as usize);
        r.skip(16)?;
    }
    let count = r.u32()? as usize;
    if count < iv_sizes.len() {
        return Err(malformed(format!("senc describes {count} samples of {}", iv_sizes.len())));
    }
    let mut infos = Vec::with_capacity(iv_sizes.len());
    for &size in iv_sizes {
        let size = if size == 0 { 0 } else { override_size.unwrap_or(size) };
        let iv = r.take(size)?.to_vec();
        let mut subsamples = Vec::new();
        if flags & 0x2 != 0 {
            for _ in 0..r.u16()? {
                subsamples.push((u32::from(r.u16()?), r.u32()?));
            }
        }
        infos.push((iv, subsamples));
    }
    Ok(infos)
}

fn read_saio(data: &[u8], boxes: &[Bx], base: usize, iv_sizes: &[usize]) -> Result<Vec<AuxInfo>> {
    let missing = || malformed("encrypted samples without senc nor saiz/saio");
    let saiz = boxes.iter().find(|b| &b.kind == b"saiz").ok_or_else(missing)?;
    let saio = boxes.iter().find(|b| &b.kind == b"saio").ok_or_else(missing)?;
    let mut r = Reader::of(data, saiz);
    let (_, flags) = r.full()?;
    if flags & 0x1 != 0 {
        r.skip(8)?;
    }
    let default_size = r.u8()?;
    let count = r.u32()? as usize;
    let sizes: Vec<usize> = if default_size == 0 { (0..count).map(|_| r.u8().map(usize::from)).collect::<Result<_>>()? } else { vec![default_size as usize; count] };
    let mut r = Reader::of(data, saio);
    let (version, flags) = r.full()?;
    if flags & 0x1 != 0 {
        r.skip(8)?;
    }
    if r.u32()? != 1 {
        return Err(DrmError::Unsupported("saio with several offsets".into()));
    }
    let offset = if version == 0 { u64::from(r.u32()?) } else { r.u64()? };
    let mut at = usize::try_from(offset).ok().and_then(|o| base.checked_add(o)).ok_or_else(|| malformed("saio offset"))?;
    if sizes.len() < iv_sizes.len() {
        return Err(malformed(format!("saiz describes {} samples of {}", sizes.len(), iv_sizes.len())));
    }
    let mut infos = Vec::with_capacity(iv_sizes.len());
    for (&iv_size, &size) in iv_sizes.iter().zip(&sizes) {
        let end = at.checked_add(size).filter(|end| *end <= data.len()).ok_or_else(|| malformed("auxiliary information beyond the segment"))?;
        let mut r = Reader::new(data, at..end);
        let iv = r.take(iv_size.min(size))?.to_vec();
        let mut subsamples = Vec::new();
        if r.left() >= 2 {
            for _ in 0..r.u16()? {
                subsamples.push((u32::from(r.u16()?), r.u32()?));
            }
        }
        infos.push((iv, subsamples));
        at = end;
    }
    Ok(infos)
}

/// Turns each box at `offsets` (the offset of its type) into `free`, keeping its size, so that every offset
/// around stays true.
pub fn neutralise(data: &mut [u8], offsets: &[usize]) {
    for &at in offsets {
        if let Some(kind) = data.get_mut(at..at + 4) {
            kind.copy_from_slice(&NEUTRAL);
        }
    }
}

/// The sample entries back to their own format, the protection boxes made `free`.
pub fn clear_init(data: &mut [u8], init: &Init) {
    for &(at, format) in &init.rename {
        if let Some(kind) = data.get_mut(at..at + 4) {
            kind.copy_from_slice(&format);
        }
    }
    neutralise(data, &init.neutral);
}

/// The `sidx` of a single-file representation: where each subsegment starts, its size, and its duration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Index {
    pub timescale: u32,
    pub earliest: u64,
    /// Byte ranges in the file, with each one's duration.
    pub references: Vec<(Range<u64>, u32)>,
}

impl Index {
    /// `data` starts with the `sidx` box, found at `at` in its file.
    pub fn parse(data: &[u8], at: u64) -> Result<Index> {
        let Some(sidx) = children(data, 0..data.len())?.into_iter().find(|b| &b.kind == b"sidx") else { return Err(malformed("no sidx box")) };
        let mut r = Reader::of(data, &sidx);
        let (version, _) = r.full()?;
        r.skip(4)?;
        let timescale = r.u32()?;
        let (earliest, first_offset) = if version == 0 { (u64::from(r.u32()?), u64::from(r.u32()?)) } else { (r.u64()?, r.u64()?) };
        r.skip(2)?;
        let count = r.u16()?;
        let mut next = at + sidx.end as u64 + first_offset;
        let mut references = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let size = r.u32()?;
            if size & 0x8000_0000 != 0 {
                return Err(DrmError::Unsupported("sidx naming other sidx".into()));
            }
            let duration = r.u32()?;
            r.skip(4)?;
            references.push((next..next + u64::from(size), duration));
            next += u64::from(size);
        }
        Ok(Index { timescale, earliest, references })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;

    #[test]
    fn boxes_of_every_size_form() {
        let mut data = bx(b"free", &[1, 2]);
        let mut large = vec![0, 0, 0, 1];
        large.extend_from_slice(b"mdat");
        large.extend_from_slice(&19u64.to_be_bytes());
        large.extend_from_slice(&[7, 7, 7]);
        data.extend_from_slice(&large);
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(b"skip");
        data.extend_from_slice(&[5]);
        let found = children(&data, 0..data.len()).unwrap();
        assert_eq!(found.iter().map(|b| (&b.kind, b.start, b.body, b.end)).collect::<Vec<_>>(), vec![(b"free", 0, 8, 10), (b"mdat", 10, 26, 29), (b"skip", 29, 37, 38)]);
        assert!(children(&bx(b"moov", &[0; 4])[..10], 0..10).is_err(), "a box larger than its parent");
    }

    #[test]
    fn init_segments_say_how_each_track_is_protected() {
        let video = TrackSpec { id: 1, audio: false, scheme: *b"cbcs", crypto: crypto([5; 16], 0, Some(vec![9; 16]), (1, 9)) };
        let audio = TrackSpec { id: 2, audio: true, scheme: *b"cenc", crypto: crypto([6; 16], 8, None, (0, 0)) };
        let data = init_segment(&[video.clone(), audio.clone()], &[Pssh::new(crate::pssh::WIDEVINE, b"wv")]);
        let init = Init::parse(&data).unwrap();
        assert_eq!(init.tracks.len(), 2);
        let entry = init.track(1).unwrap().entries[0].clone().unwrap();
        assert_eq!((entry.format, entry.scheme, entry.crypto), (*b"avc1", *b"cbcs", video.crypto));
        let entry = init.track(2).unwrap().entries[0].clone().unwrap();
        assert_eq!((entry.format, entry.scheme, entry.crypto), (*b"mp4a", *b"cenc", audio.crypto));
        assert_eq!(init.pssh, vec![Pssh::new(crate::pssh::WIDEVINE, b"wv")]);
        assert_eq!(init.kids(), vec![[5; 16], [6; 16]]);
        assert!(init.is_protected());

        let mut cleared = data.clone();
        clear_init(&mut cleared, &init);
        assert_eq!(cleared.len(), data.len());
        let again = Init::parse(&cleared).unwrap();
        assert!(!again.is_protected() && again.pssh.is_empty());
        assert_eq!(again.tracks[0].entries, vec![None]);
        assert!(find(&cleared, b"avc1") && find(&cleared, b"mp4a") && !find(&cleared, b"encv") && !find(&cleared, b"sinf") && !find(&cleared, b"pssh"));
    }

    #[test]
    fn media_segments_say_where_each_sample_is_and_how() {
        let spec = TrackSpec { id: 1, audio: false, scheme: *b"cenc", crypto: crypto([5; 16], 8, None, (0, 0)) };
        let init = Init::parse(&init_segment(std::slice::from_ref(&spec), &[])).unwrap();
        let samples = vec![
            SampleSpec { data: vec![1; 40], iv: vec![1; 8], subsamples: vec![(4, 36)] },
            SampleSpec { data: vec![2; 20], iv: vec![2; 8], subsamples: vec![] },
        ];
        for aux in [Aux::Senc, Aux::SaizSaio] {
            let data = media_segment(&spec, &samples, aux, &[Pssh::new(crate::pssh::WIDEVINE, b"next")]);
            let segment = Segment::parse(&data, &init, 0).unwrap();
            assert_eq!(segment.encrypted.len(), 2, "{aux:?}");
            let mdat = data.windows(4).position(|w| w == b"mdat").unwrap() + 4;
            assert_eq!(segment.encrypted[0].offset, mdat);
            assert_eq!(segment.encrypted[1].offset, mdat + 40);
            let first = &segment.encrypted[0].sample;
            assert_eq!((first.size, first.scheme, &first.kid[..], first.subsamples.clone()), (40, Scheme::Cenc, &[5; 16][..], vec![(4, 36)]));
            assert_eq!(first.iv, [vec![1; 8], vec![0; 8]].concat(), "an 8-byte IV is the high half");
            assert_eq!(segment.encrypted[1].sample.subsamples, vec![]);
            assert_eq!(segment.pssh.len(), 1);
            assert_eq!(segment.kids(), vec![vec![5; 16]]);
            let mut cleared = data.clone();
            neutralise(&mut cleared, &segment.neutral);
            assert!(!find(&cleared, b"senc") && !find(&cleared, b"saiz") && !find(&cleared, b"saio") && !find(&cleared, b"pssh"), "{aux:?}");
            assert!(Segment::parse(&cleared, &init, 0).is_err(), "encrypted samples need their auxiliary information");
        }
    }

    #[test]
    fn sample_groups_override_the_track_defaults() {
        let spec = TrackSpec { id: 1, audio: false, scheme: *b"cbcs", crypto: crypto([5; 16], 0, Some(vec![3; 16]), (1, 9)) };
        let init = Init::parse(&init_segment(std::slice::from_ref(&spec), &[])).unwrap();
        let samples: Vec<SampleSpec> = (0..3).map(|i| SampleSpec { data: vec![i; 32], iv: vec![], subsamples: vec![(0, 32)] }).collect();
        let rotated = crypto([8; 16], 0, Some(vec![4; 16]), (1, 9));
        let clear = Crypto { protected: false, iv_size: 0, kid: [0; 16], constant_iv: vec![], pattern: (0, 0) };
        let data = media_segment_with_groups(&spec, &samples, &[rotated.clone(), clear], &[(1, 0), (1, 0x10001), (1, 0x10002)]);
        let segment = Segment::parse(&data, &init, 0).unwrap();
        let kids: Vec<_> = segment.encrypted.iter().map(|e| (e.sample.kid[0], e.sample.iv[0], e.sample.pattern)).collect();
        assert_eq!(kids, vec![(5, 3, Some((1, 9))), (8, 4, Some((1, 9)))], "the third sample is clear");
        assert_eq!(segment.neutral.len(), 3, "sgpd, sbgp and senc");
    }

    #[test]
    fn other_schemes_are_refused_by_name() {
        let spec = TrackSpec { id: 1, audio: false, scheme: *b"cens", crypto: crypto([5; 16], 16, None, (1, 9)) };
        let init = Init::parse(&init_segment(std::slice::from_ref(&spec), &[])).unwrap();
        let data = media_segment(&spec, &[SampleSpec { data: vec![0; 16], iv: vec![0; 16], subsamples: vec![] }], Aux::Senc, &[]);
        assert_eq!(Segment::parse(&data, &init, 0), Err(DrmError::Unsupported("cens encryption".into())));
    }

    #[test]
    fn single_file_indexes_list_their_subsegments() {
        let mut body = vec![0, 0, 0, 0];
        body.extend_from_slice(&1u32.to_be_bytes());
        body.extend_from_slice(&90000u32.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes());
        body.extend_from_slice(&10u32.to_be_bytes());
        body.extend_from_slice(&[0, 0, 0, 2]);
        for (size, duration) in [(1000u32, 180000u32), (500, 90000)] {
            body.extend_from_slice(&size.to_be_bytes());
            body.extend_from_slice(&duration.to_be_bytes());
            body.extend_from_slice(&[0x90, 0, 0, 0]);
        }
        let sidx = bx(b"sidx", &body);
        let index = Index::parse(&sidx, 800).unwrap();
        let start = 800 + sidx.len() as u64 + 10;
        assert_eq!(index, Index { timescale: 90000, earliest: 0, references: vec![(start..start + 1000, 180000), (start + 1000..start + 1500, 90000)] });
    }
}

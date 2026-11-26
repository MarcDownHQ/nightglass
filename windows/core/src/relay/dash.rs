//! DASH manifests (ISO/IEC 23009-1) of protected streams: what they say about keys and segments, and the same
//! manifest without protection, every segment named through the relay.

use nightglass_drm::mp4::Index;
use nightglass_drm::pssh::{Pssh, WIDEVINE};
use url::Url;

use crate::xml::{self, Element, Node};

const WIDEVINE_SCHEME: &str = "urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed";

/// Inclusive.
pub(super) type ByteRange = (u64, u64);

/// Where a segment is: an address, and a byte range in it.
pub(super) type Source = (Url, Option<ByteRange>);

/// A representation as the relay needs it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Rep {
    /// Unique in the manifest and stable across its refreshes.
    pub track: String,
    /// Where its segment addresses resolve.
    pub base: Url,
    pub kid: Option<[u8; 16]>,
    pub init: Option<Source>,
    /// A single file indexed by its `sidx`: byte range of the index, and of the init segment.
    pub single: Option<(ByteRange, Option<ByteRange>)>,
    /// Its `BaseURL` alone addresses it: one resource, such as subtitles.
    pub whole: bool,
    pub video: bool,
}

pub(super) struct Manifest {
    nodes: Vec<Node>,
    url: Url,
    pub pssh: Vec<Pssh>,
    pub reps: Vec<Rep>,
}

/// What the relay serves in place of a representation's own addressing.
pub(super) enum Addressing {
    /// Its segments resolve against this address, the relay's stand-in for `Rep::base`.
    Base(Url),
    /// A single file, as a list of segments at these addresses, init first.
    List { index: Index, init: Url, media: Vec<Url> },
}

impl Manifest {
    pub fn parse(text: &str, url: &Url) -> Option<Manifest> {
        let mut nodes = xml::parse(text.trim_start_matches('\u{feff}'))?;
        let root = xml::root_mut(&mut nodes)?;
        if root.name() != "MPD" {
            return None;
        }
        let mut reps = Vec::new();
        let pssh = walk(&mut root.clone(), url, false, &mut |rep, _| {
            reps.push(rep.clone());
            true
        });
        Some(Manifest { nodes, url: url.clone(), pssh, reps })
    }

    /// The manifest without protection: representations `keep` refuses are left out (and adaptation sets left
    /// without any), the others addressed as `address` says.
    pub fn rewrite(mut self, keep: impl Fn(&Rep) -> bool, mut address: impl FnMut(&Rep) -> Addressing) -> String {
        let url = self.url.clone();
        if let Some(root) = xml::root_mut(&mut self.nodes) {
            walk(root, &url, true, &mut |rep, element| {
                if !keep(rep) {
                    return false;
                }
                match address(rep) {
                    Addressing::Base(base) => {
                        let mut base_url = Element::new("BaseURL");
                        base_url.set_text(base.as_str());
                        element.prepend(base_url);
                    }
                    Addressing::List { index, init, media } => {
                        element.remove("SegmentBase");
                        element.append(segment_list(&index, &init, &media));
                    }
                }
                true
            });
        }
        xml::write(&self.nodes)
    }
}

/// The base address `element`'s first `BaseURL` gives, against `parent`.
fn base_of(element: &Element, parent: &Url) -> Url {
    element.child("BaseURL").and_then(|b| parent.join(b.text().trim()).ok()).unwrap_or_else(|| parent.clone())
}

/// The default key and the Widevine `pssh` boxes of an element's `ContentProtection`s.
fn protection(element: &Element) -> (Option<[u8; 16]>, Vec<Pssh>) {
    let (mut kid, mut pssh) = (None, Vec::new());
    for cp in element.elements().filter(|e| e.name() == "ContentProtection") {
        if let Some(found) = cp.attr("default_KID").and_then(|k| nightglass_drm::unhex(&k)).and_then(|k| k.try_into().ok()) {
            kid = Some(found);
        }
        if cp.attr("schemeIdUri").is_some_and(|s| s.eq_ignore_ascii_case(WIDEVINE_SCHEME)) {
            pssh.extend(cp.elements().filter(|e| e.name() == "pssh").filter_map(|p| Pssh::from_base64(&p.text(), WIDEVINE).ok()));
        }
    }
    (kid, pssh)
}

/// The nearest of the levels' `name` elements that has `attr`.
fn inherited(levels: &[Option<&Element>], attr: &str) -> Option<String> {
    levels.iter().flatten().find_map(|e| e.attr(attr))
}

fn range(text: &str) -> Option<(u64, u64)> {
    let (start, end) = text.trim().split_once('-')?;
    Some((start.trim().parse().ok()?, end.trim().parse().ok()?))
}

/// A template with only the identifiers an init segment may use.
fn expand(template: &str, id: &str, bandwidth: Option<&str>) -> Option<String> {
    let mut out = String::new();
    let mut parts = template.split('$');
    out.push_str(parts.next()?);
    while let Some(name) = parts.next() {
        match name {
            "" => out.push('$'),
            "RepresentationID" => out.push_str(id),
            "Bandwidth" => out.push_str(bandwidth?),
            _ => return None,
        }
        out.push_str(parts.next()?);
    }
    Some(out)
}

fn is_absolute(address: &str) -> bool {
    Url::parse(address).is_ok()
}

/// The representations of `root` with what the relay needs to know of them, each passed to `f` with its element;
/// those `f` refuses are removed. With `strip`, protection and addresses go: `ContentProtection`, `BaseURL`,
/// `Location`. The Widevine `pssh` boxes met on the way come back.
fn walk(root: &mut Element, url: &Url, strip: bool, f: &mut dyn FnMut(&Rep, &mut Element) -> bool) -> Vec<Pssh> {
    let mut pssh = Vec::new();
    let base0 = base_of(root, url);
    for (pi, period) in root.elements_mut().filter(|e| e.name() == "Period").enumerate() {
        let base1 = base_of(period, &base0);
        let period_id = period.attr("id").unwrap_or_else(|| pi.to_string());
        let (period_template, period_list, period_base) = (period.child("SegmentTemplate").cloned(), period.child("SegmentList").cloned(), period.child("SegmentBase").cloned());
        let mut period_single = false;
        for (ai, set) in period.elements_mut().filter(|e| e.name() == "AdaptationSet").enumerate() {
            let base2 = base_of(set, &base1);
            let (set_kid, set_pssh) = protection(set);
            let set_video = ["contentType", "mimeType"].iter().any(|a| set.attr(a).is_some_and(|v| v.starts_with("video")));
            pssh.extend(set_pssh);
            let (set_template, set_list, set_base) = (set.child("SegmentTemplate").cloned(), set.child("SegmentList").cloned(), set.child("SegmentBase").cloned());
            let mut set_single = false;
            let mut ri = 0;
            set.children.retain_mut(|node| {
                let Node::Element(element) = node else { return true };
                if element.name() != "Representation" {
                    return true;
                }
                ri += 1;
                let base3 = base_of(element, &base2);
                let (kid, rep_pssh) = protection(element);
                pssh.extend(rep_pssh);
                let id = element.attr("id").unwrap_or_else(|| (ri - 1).to_string());
                let bandwidth = element.attr("bandwidth");
                let templates = [element.child("SegmentTemplate"), set_template.as_ref(), period_template.as_ref()];
                let lists = [element.child("SegmentList"), set_list.as_ref(), period_list.as_ref()];
                let bases = [element.child("SegmentBase"), set_base.as_ref(), period_base.as_ref()];
                let has_template = templates.iter().any(Option::is_some);
                let list = lists.iter().flatten().next();
                let segment_base = bases.iter().flatten().next();
                let mut init = None;
                let mut single = None;
                let mut absolute_template = false;
                if has_template {
                    let initialization = inherited(&templates, "initialization");
                    let media = inherited(&templates, "media");
                    absolute_template = initialization.iter().chain(&media).any(|a| is_absolute(a));
                    init = initialization.and_then(|t| expand(&t, &id, bandwidth.as_deref())).and_then(|t| base3.join(&t).ok()).map(|u| (u, None));
                } else if let Some(list) = list {
                    if let Some(i) = list.child("Initialization") {
                        let address = i.attr("sourceURL").and_then(|s| base3.join(&s).ok()).unwrap_or_else(|| base3.clone());
                        init = Some((address, i.attr("range").and_then(|r| range(&r))));
                    }
                } else if let Some(segment_base) = segment_base {
                    let init_range = segment_base.child("Initialization").and_then(|i| i.attr("range")).and_then(|r| range(&r));
                    init = init_range.map(|r| (base3.clone(), Some(r)));
                    single = segment_base.attr("indexRange").and_then(|r| range(&r)).map(|index| (index, init_range));
                }
                let base = if absolute_template {
                    let template = inherited(&templates, "media").unwrap_or_default();
                    Url::parse(&template).ok().and_then(|u| u.join("/").ok()).unwrap_or_else(|| base3.clone())
                } else {
                    base3.clone()
                };
                let relative_template = absolute_template.then(|| templates.iter().flatten().next().map(|t| (*t).clone())).flatten().map(|mut own| {
                    for attr in ["media", "initialization"] {
                        if let Some(address) = inherited(&templates, attr) {
                            own.set_attr(attr, &from_root(&address));
                        }
                    }
                    own
                });
                let whole = !has_template && list.is_none() && single.is_none();
                let video = set_video || element.attr("mimeType").is_some_and(|m| m.starts_with("video"));
                let rep = Rep { track: format!("{period_id}/{ai}/{id}"), base, kid: kid.or(set_kid), init, single, whole, video };
                if strip {
                    element.remove("ContentProtection");
                    element.remove("BaseURL");
                    if let Some(own) = relative_template {
                        element.remove("SegmentTemplate");
                        element.append(own);
                    }
                }
                let kept = f(&rep, element);
                if kept && rep.single.is_some() {
                    set_single = true;
                }
                kept
            });
            if strip {
                set.remove("ContentProtection");
                set.remove("BaseURL");
                if set_single {
                    set.remove("SegmentBase");
                    period_single = true;
                }
            }
        }
        if strip {
            period.retain(|e| e.name() != "AdaptationSet" || e.elements().any(|r| r.name() == "Representation"));
            period.remove("BaseURL");
            if period_single {
                period.remove("SegmentBase");
            }
        }
    }
    if strip {
        for name in ["BaseURL", "Location", "PatchLocation"] {
            root.remove(name);
        }
    }
    pssh
}

/// `address` from the root of its server, when it is absolute.
fn from_root(address: &str) -> String {
    match Url::parse(address) {
        Ok(url) => format!("{}{}", url.path().trim_start_matches('/'), url.query().map(|q| format!("?{q}")).unwrap_or_default()),
        Err(_) => address.to_string(),
    }
}

/// A single-file representation as a list of segments, timed by its index.
fn segment_list(index: &Index, init: &Url, media: &[Url]) -> Element {
    let mut list = Element::new("SegmentList");
    list.set_attr("timescale", &index.timescale.to_string());
    list.set_attr("presentationTimeOffset", &index.earliest.to_string());
    let mut initialization = Element::new("Initialization");
    initialization.set_attr("sourceURL", init.as_str());
    list.append(initialization);
    let mut timeline = Element::new("SegmentTimeline");
    let mut t = index.earliest;
    for (_, duration) in &index.references {
        let mut s = Element::new("S");
        s.set_attr("t", &t.to_string());
        s.set_attr("d", &duration.to_string());
        timeline.append(s);
        t += u64::from(*duration);
    }
    list.append(timeline);
    for address in media {
        let mut segment = Element::new("SegmentURL");
        segment.set_attr("media", address.as_str());
        list.append(segment);
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    const MPD: &str = r#"<?xml version="1.0"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" xmlns:cenc="urn:mpeg:cenc:2013" type="static">
  <Location>https://cdn.test/live.mpd</Location>
  <BaseURL>https://cdn.test/v/</BaseURL>
  <Period id="p0">
    <AdaptationSet mimeType="video/mp4">
      <ContentProtection schemeIdUri="urn:mpeg:dash:mp4protection:2011" value="cenc" cenc:default_KID="05050505-0505-0505-0505-050505050505"/>
      <ContentProtection schemeIdUri="urn:uuid:EDEF8BA9-79D6-4ACE-A3C8-27DCD51D21ED"><cenc:pssh>AAAAIHBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAAAA=</cenc:pssh></ContentProtection>
      <SegmentTemplate initialization="$RepresentationID$/init.mp4" media="$RepresentationID$/$Number$.m4s?t=1" startNumber="1" duration="2" timescale="1"/>
      <Representation id="hd" bandwidth="5000000"><ContentProtection schemeIdUri="urn:mpeg:dash:mp4protection:2011" cenc:default_KID="06060606-0606-0606-0606-060606060606"/></Representation>
      <Representation id="sd" bandwidth="1000000"><BaseURL>sd-cdn/</BaseURL></Representation>
    </AdaptationSet>
    <AdaptationSet mimeType="audio/mp4">
      <Representation id="a" bandwidth="128000">
        <BaseURL>https://other.test/audio.mp4</BaseURL>
        <SegmentBase indexRange="800-899"><Initialization range="0-799"/></SegmentBase>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;

    fn url() -> Url {
        Url::parse("https://origin.test/stream/manifest.mpd").unwrap()
    }

    #[test]
    fn representations_say_where_their_segments_and_keys_are() {
        let manifest = Manifest::parse(MPD, &url()).unwrap();
        assert_eq!(manifest.pssh.len(), 1);
        assert!(manifest.pssh[0].is_widevine());
        let reps: Vec<_> = manifest.reps.iter().map(|r| (r.track.as_str(), r.base.as_str(), r.kid.map(|k| k[0]), r.init.clone(), r.single)).collect();
        let source = |u: &str, r| Some((Url::parse(u).unwrap(), r));
        assert_eq!(
            reps,
            vec![
                ("p0/0/hd", "https://cdn.test/v/", Some(6), source("https://cdn.test/v/hd/init.mp4", None), None),
                ("p0/0/sd", "https://cdn.test/v/sd-cdn/", Some(5), source("https://cdn.test/v/sd-cdn/sd/init.mp4", None), None),
                ("p0/1/a", "https://other.test/audio.mp4", None, source("https://other.test/audio.mp4", Some((0, 799))), Some(((800, 899), Some((0, 799))))),
            ]
        );
        assert!(manifest.reps.iter().all(|r| !r.whole));
        assert_eq!(manifest.reps.iter().map(|r| r.video).collect::<Vec<_>>(), [true, true, false]);
        let subtitles = Manifest::parse(r#"<MPD><Period><AdaptationSet mimeType="text/vtt"><Representation id="s"><BaseURL>en.vtt</BaseURL></Representation></AdaptationSet></Period></MPD>"#, &url()).unwrap();
        assert!(subtitles.reps[0].whole && subtitles.reps[0].base.as_str() == "https://origin.test/stream/en.vtt");
        assert!(Manifest::parse("<html/>", &url()).is_none());
    }

    #[test]
    fn the_rewritten_manifest_names_the_relay_and_no_protection() {
        let manifest = Manifest::parse(MPD, &url()).unwrap();
        let local = |n: &str| Url::parse(&format!("http://127.0.0.1:1/t/{n}")).unwrap();
        let rewritten = manifest.rewrite(
            |rep| rep.kid != Some([6; 16]),
            |rep| match rep.single {
                None => Addressing::Base(local(&format!("{}/", rep.track.replace('/', "-")))),
                Some(_) => Addressing::List { index: Index { timescale: 1000, earliest: 0, references: vec![(900..1900, 2000), (1900..2400, 1000)] }, init: local("i.mp4"), media: vec![local("s0.m4s"), local("s1.m4s")] },
            },
        );
        for gone in ["ContentProtection", "cenc:pssh", "Location", "https://cdn.test", "https://other.test", "hd/", "SegmentBase"] {
            assert!(!rewritten.contains(gone), "{gone} in {rewritten}");
        }
        assert!(rewritten.contains(r#"<Representation id="sd" bandwidth="1000000"><BaseURL>http://127.0.0.1:1/t/p0-0-sd/</BaseURL></Representation>"#), "{rewritten}");
        assert!(rewritten.contains(r#"media="$RepresentationID$/$Number$.m4s?t=1""#), "templates stay as they were");
        assert!(rewritten.contains(r#"<SegmentList timescale="1000" presentationTimeOffset="0"><Initialization sourceURL="http://127.0.0.1:1/t/i.mp4"/><SegmentTimeline><S t="0" d="2000"/><S t="2000" d="1000"/></SegmentTimeline><SegmentURL media="http://127.0.0.1:1/t/s0.m4s"/><SegmentURL media="http://127.0.0.1:1/t/s1.m4s"/></SegmentList>"#), "{rewritten}");
        let again = Manifest::parse(&rewritten, &url()).unwrap();
        assert_eq!(again.reps.len(), 2);
        assert!(again.pssh.is_empty());
    }

    #[test]
    fn a_set_with_no_representation_left_goes() {
        let manifest = Manifest::parse(MPD, &url()).unwrap();
        let rewritten = manifest.rewrite(|rep| rep.kid.is_none(), |_| Addressing::Base(Url::parse("http://127.0.0.1:1/t/b/").unwrap()));
        assert!(!rewritten.contains("video/mp4") && rewritten.contains("audio/mp4"), "{rewritten}");
    }

    #[test]
    fn absolute_templates_are_named_from_the_root_of_their_server() {
        let mpd = r#"<MPD><Period><AdaptationSet><SegmentTemplate initialization="https://cdn.test/a/$RepresentationID$-init.mp4" media="https://cdn.test/a/$RepresentationID$-$Number$.m4s?k=v"/><Representation id="r"/></AdaptationSet></Period></MPD>"#;
        let manifest = Manifest::parse(mpd, &url()).unwrap();
        assert_eq!(manifest.reps[0].base.as_str(), "https://cdn.test/");
        assert_eq!(manifest.reps[0].init.as_ref().unwrap().0.as_str(), "https://cdn.test/a/r-init.mp4");
        let rewritten = manifest.rewrite(|_| true, |_| Addressing::Base(Url::parse("http://127.0.0.1:1/t/b/").unwrap()));
        assert!(rewritten.contains(r#"<Representation id="r"><BaseURL>http://127.0.0.1:1/t/b/</BaseURL><SegmentTemplate initialization="a/$RepresentationID$-init.mp4" media="a/$RepresentationID$-$Number$.m4s?k=v"/></Representation>"#), "{rewritten}");
    }

    #[test]
    fn init_templates_take_only_what_an_init_can_name() {
        assert_eq!(expand("$RepresentationID$_$Bandwidth$$$.mp4", "v1", Some("800")), Some("v1_800$.mp4".into()));
        assert_eq!(expand("$Number$.mp4", "v1", None), None);
        assert_eq!(expand("$Bandwidth$.mp4", "v1", None), None);
    }
}

//! What plugins keep between calls. Each plugin has its own keys; the persistent ones live in `<dir>/<plugin id>.json`,
//! written whenever one of them changes. The calls block on the disk: the host runs them off the async workers.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MAX_KEY: usize = 256;
pub const MAX_VALUE: usize = 1 << 20;
/// Keys and values of one plugin, persistent or not.
pub const MAX_TOTAL: usize = 16 << 20;

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    value: String,
    /// Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires: Option<i64>,
    #[serde(skip)]
    persist: bool,
}

impl Entry {
    fn live(&self, now: i64) -> bool {
        self.expires.is_none_or(|e| e > now)
    }
}

#[derive(Default)]
struct Space {
    loaded: bool,
    entries: HashMap<String, Entry>,
}

impl Space {
    fn size(&self) -> usize {
        self.entries.iter().map(|(k, e)| k.len() + e.value.len()).sum()
    }
}

pub struct Storage {
    dir: Option<PathBuf>,
    spaces: Mutex<HashMap<String, Arc<Mutex<Space>>>>,
    clock: fn() -> i64,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

impl Storage {
    /// Persistent values are kept in `dir`, or only in memory without one.
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self::with_clock(dir, now)
    }

    fn with_clock(dir: Option<PathBuf>, clock: fn() -> i64) -> Self {
        Storage { dir, spaces: Mutex::default(), clock }
    }

    fn space(&self, plugin: &str) -> Arc<Mutex<Space>> {
        self.spaces.lock().unwrap().entry(plugin.into()).or_default().clone()
    }

    fn file(&self, plugin: &str) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(format!("{plugin}.json")))
    }

    /// A file that can't be read leaves the space unloaded, so that nothing overwrites it; one that doesn't parse is
    /// moved aside.
    fn load(&self, plugin: &str, space: &mut Space) -> Result<(), String> {
        if space.loaded {
            return Ok(());
        }
        let Some(file) = self.file(plugin) else {
            space.loaded = true;
            return Ok(());
        };
        let bytes = match std::fs::read(&file) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                space.loaded = true;
                return Ok(());
            }
            Err(e) => {
                log::warn!("{}: {e}", file.display());
                return Err(format!("storage unreadable: {e}"));
            }
        };
        space.loaded = true;
        let entries: HashMap<String, Entry> = match serde_json::from_slice(&bytes) {
            Ok(entries) => entries,
            Err(e) => {
                let aside = file.with_extension(format!("{}.json.bad", (self.clock)()));
                log::warn!("{}: {e}, moved to {}", file.display(), aside.display());
                if let Err(e) = std::fs::rename(&file, &aside) {
                    log::warn!("{}: {e}", aside.display());
                }
                return Ok(());
            }
        };
        let now = (self.clock)();
        space.entries.extend(entries.into_iter().filter(|(_, e)| e.live(now)).map(|(k, e)| (k, Entry { persist: true, ..e })));
        Ok(())
    }

    fn save(&self, plugin: &str, space: &Space) -> Result<(), String> {
        let Some(file) = self.file(plugin) else { return Ok(()) };
        let now = (self.clock)();
        let kept: HashMap<&String, &Entry> = space.entries.iter().filter(|(_, e)| e.persist && e.live(now)).collect();
        write(&file, &serde_json::to_vec(&kept).expect("strings serialize")).map_err(|e| {
            log::warn!("{}: {e}", file.display());
            format!("storage not written: {e}")
        })
    }

    pub fn get(&self, plugin: &str, key: &str) -> Option<String> {
        let space = self.space(plugin);
        let mut space = space.lock().unwrap();
        self.load(plugin, &mut space).ok()?;
        let entry = space.entries.get(key)?;
        if entry.live((self.clock)()) {
            return Some(entry.value.clone());
        }
        space.entries.remove(key);
        None
    }

    /// `ttl` in seconds. A persistent value that can't be written is not kept.
    pub fn set(&self, plugin: &str, key: String, value: String, ttl: Option<u32>, persist: bool) -> Result<(), String> {
        if key.len() > MAX_KEY {
            return Err(format!("key longer than {MAX_KEY} bytes"));
        }
        if value.len() > MAX_VALUE {
            return Err(format!("value longer than {MAX_VALUE} bytes"));
        }
        let space = self.space(plugin);
        let mut space = space.lock().unwrap();
        self.load(plugin, &mut space)?;
        let now = (self.clock)();
        space.entries.retain(|_, e| e.live(now));
        let replaced = space.entries.get(&key).map_or(0, |e| key.len() + e.value.len());
        if space.size() - replaced + key.len() + value.len() > MAX_TOTAL {
            return Err(format!("over {MAX_TOTAL} bytes stored"));
        }
        let entry = Entry { value, expires: ttl.map(|t| now + i64::from(t)), persist };
        let previous = space.entries.insert(key.clone(), entry);
        if (persist || previous.as_ref().is_some_and(|e| e.persist))
            && let Err(e) = self.save(plugin, &space)
        {
            match previous {
                Some(previous) => space.entries.insert(key, previous),
                None => space.entries.remove(&key),
            };
            return Err(e);
        }
        Ok(())
    }

    pub fn delete(&self, plugin: &str, key: &str) {
        let space = self.space(plugin);
        let mut space = space.lock().unwrap();
        if self.load(plugin, &mut space).is_err() {
            return;
        }
        if space.entries.remove(key).is_some_and(|e| e.persist) {
            let _ = self.save(plugin, &space);
        }
    }
}

fn write(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    let mut out = std::fs::File::create(&tmp)?;
    out.write_all(bytes)?;
    out.sync_all()?;
    std::fs::rename(&tmp, file)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    static NOW: AtomicI64 = AtomicI64::new(1_000);

    fn clock() -> i64 {
        NOW.load(Ordering::SeqCst)
    }

    fn dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nightglass-storage-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn rand_suffix() -> u64 {
        static N: AtomicI64 = AtomicI64::new(0);
        N.fetch_add(1, Ordering::SeqCst) as u64
    }

    #[test]
    fn plugins_see_only_their_own_keys() {
        let storage = Storage::new(None);
        storage
            .set("a", "k".into(), "1".into(), None, false)
            .unwrap();
        assert_eq!(storage.get("a", "k").as_deref(), Some("1"));
        assert_eq!(storage.get("b", "k"), None);
        storage.delete("a", "k");
        assert_eq!(storage.get("a", "k"), None);
    }

    #[test]
    fn values_expire() {
        let storage = Storage::with_clock(None, clock);
        let start = clock();
        storage
            .set("a", "k".into(), "1".into(), Some(10), false)
            .unwrap();
        NOW.store(start + 9, Ordering::SeqCst);
        assert_eq!(storage.get("a", "k").as_deref(), Some("1"));
        NOW.store(start + 10, Ordering::SeqCst);
        assert_eq!(storage.get("a", "k"), None);
    }

    #[test]
    fn only_persistent_values_survive_a_restart() {
        let dir = dir();
        let storage = Storage::new(Some(dir.clone()));
        storage
            .set("a", "kept".into(), "1".into(), None, true)
            .unwrap();
        storage
            .set("a", "session".into(), "2".into(), None, false)
            .unwrap();
        storage
            .set("a", "dropped".into(), "3".into(), None, true)
            .unwrap();
        storage.delete("a", "dropped");
        let storage = Storage::new(Some(dir.clone()));
        assert_eq!(storage.get("a", "kept").as_deref(), Some("1"));
        assert_eq!(storage.get("a", "session"), None);
        assert_eq!(storage.get("a", "dropped"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_value_set_again_without_persist_leaves_the_disk() {
        let dir = dir();
        let storage = Storage::new(Some(dir.clone()));
        storage
            .set("a", "k".into(), "1".into(), None, true)
            .unwrap();
        storage
            .set("a", "k".into(), "2".into(), None, false)
            .unwrap();
        assert_eq!(Storage::new(Some(dir.clone())).get("a", "k"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unreadable_file_is_moved_aside() {
        let dir = dir();
        std::fs::write(dir.join("a.json"), "not json").unwrap();
        let storage = Storage::new(Some(dir.clone()));
        assert_eq!(storage.get("a", "k"), None);
        storage
            .set("a", "k".into(), "1".into(), None, true)
            .unwrap();
        let aside: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).filter(|n| n.ends_with(".json.bad")).collect();
        assert_eq!(aside.len(), 1, "{aside:?}");
        assert_eq!(std::fs::read_to_string(dir.join(&aside[0])).unwrap(), "not json");
        assert_eq!(
            Storage::new(Some(dir.clone())).get("a", "k").as_deref(),
            Some("1")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_that_cannot_be_read_is_left_alone() {
        let dir = dir();
        std::fs::create_dir(dir.join("a.json")).unwrap();
        let storage = Storage::new(Some(dir.clone()));
        assert_eq!(storage.get("a", "k"), None);
        assert!(storage.set("a", "k".into(), "1".into(), None, false).is_err());
        storage.delete("a", "k");
        assert!(dir.join("a.json").is_dir());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_persistent_value_that_cannot_be_written_is_not_kept() {
        let dir = dir();
        let storage = Storage::new(Some(dir.clone()));
        std::fs::create_dir(dir.join("a.json.tmp")).unwrap();
        storage.set("a", "k".into(), "1".into(), None, false).unwrap();
        assert!(storage.set("a", "k".into(), "2".into(), None, true).is_err());
        assert_eq!(storage.get("a", "k").as_deref(), Some("1"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sizes_are_capped() {
        let storage = Storage::new(None);
        assert!(
            storage
                .set("a", "k".repeat(MAX_KEY + 1), String::new(), None, false)
                .is_err()
        );
        assert!(
            storage
                .set("a", "k".into(), "v".repeat(MAX_VALUE + 1), None, false)
                .is_err()
        );
        for i in 0..MAX_TOTAL / MAX_VALUE - 1 {
            storage
                .set("a", format!("{i}"), "v".repeat(MAX_VALUE), None, false)
                .unwrap();
        }
        assert!(
            storage
                .set("a", "last".into(), "v".repeat(MAX_VALUE), None, false)
                .is_err()
        );
        storage
            .set("a", "0".into(), "v".repeat(MAX_VALUE), None, false)
            .expect("replacing a value counts only the difference");
    }
}

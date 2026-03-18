use crate::app::DirectoryState;
use crate::types::{Node, NodeKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedNode {
    path: String,
    size: u64,
    kind: CachedNodeKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum CachedNodeKind {
    Directory,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDirectory {
    scanned_at_unix_secs: u64,
    children: Vec<CachedNode>,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CacheFile {
    version: u32,
    dirs: HashMap<String, CachedDirectory>,
}

#[derive(Debug, Clone)]
pub struct CacheStore {
    path: PathBuf,
    data: CacheFile,
}

impl CacheStore {
    pub fn load_default() -> Self {
        let path = default_cache_path();
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<CacheFile>(&text).ok())
            .filter(|cache| cache.version == 1)
            .unwrap_or_else(|| CacheFile {
                version: 1,
                dirs: HashMap::new(),
            });

        Self { path, data }
    }

    pub fn get_fresh_directory_state(&mut self, path: &Path) -> Option<DirectoryState> {
        let key = normalize_key(path);
        let entry = self.data.dirs.get(&key)?.clone();

        if !is_snapshot_fresh(path, entry.scanned_at_unix_secs) {
            self.data.dirs.remove(&key);
            return None;
        }

        Some(DirectoryState {
            children: entry
                .children
                .into_iter()
                .map(|n| Node {
                    path: PathBuf::from(n.path),
                    size: n.size,
                    kind: match n.kind {
                        CachedNodeKind::Directory => NodeKind::Directory,
                        CachedNodeKind::File => NodeKind::File,
                    },
                })
                .collect(),
            size: entry.size,
            loaded_size: entry.size,
            size_locked: true,
            loading: false,
            loaded: true,
            error: None,
        })
    }

    pub fn put_directory_state(&mut self, path: &Path, state: &DirectoryState) {
        if state.loading || !state.loaded || state.error.is_some() {
            return;
        }

        let key = normalize_key(path);
        let scanned_at_unix_secs = now_unix_secs();
        let children = state
            .children
            .iter()
            .map(|n| CachedNode {
                path: n.path.to_string_lossy().into_owned(),
                size: n.size,
                kind: match n.kind {
                    NodeKind::Directory => CachedNodeKind::Directory,
                    NodeKind::File => CachedNodeKind::File,
                },
            })
            .collect::<Vec<_>>();

        self.data.dirs.insert(
            key,
            CachedDirectory {
                scanned_at_unix_secs,
                children,
                size: state.size,
            },
        );
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create cache dir {}: {e}", parent.display()))?;
        }

        let text = serde_json::to_string_pretty(&self.data)
            .map_err(|e| format!("failed to serialize cache: {e}"))?;
        fs::write(&self.path, text)
            .map_err(|e| format!("failed to write cache {}: {e}", self.path.display()))
    }
}

fn default_cache_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("memblocks")
            .join("cache.json")
    } else {
        PathBuf::from(".memblocks-cache.json")
    }
}

fn normalize_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn is_snapshot_fresh(path: &Path, scanned_at_unix_secs: u64) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }

    let mut saw_any_path = false;
    for entry in WalkDir::new(path).follow_links(false) {
        let Ok(entry) = entry else {
            return false;
        };
        let entry_path = entry.path();
        saw_any_path = true;

        let Ok(meta) = fs::symlink_metadata(entry_path) else {
            return false;
        };
        let Ok(modified) = meta.modified() else {
            return false;
        };
        let modified_unix_secs = modified
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if modified_unix_secs > scanned_at_unix_secs {
            return false;
        }
    }

    // WalkDir already captures creation/deletion changes through directory mtime changes,
    // so no extra child-list checksum is needed for startup freshness checks.
    saw_any_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let unique = format!(
            "diskblock-cache-test-{}-{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        dir.push(unique);
        fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn write_text(path: &Path, text: &str) {
        fs::write(path, text).expect("write file");
    }

    #[test]
    fn snapshot_is_fresh_without_changes() {
        let dir = make_temp_dir("fresh");
        let file = dir.join("a.txt");
        write_text(&file, "hello");

        let scanned_at = now_unix_secs().saturating_add(5);
        assert!(is_snapshot_fresh(&dir, scanned_at));

        fs::remove_dir_all(&dir).expect("cleanup temp directory");
    }

    #[test]
    fn snapshot_is_stale_after_file_modification() {
        let dir = make_temp_dir("modify");
        let file = dir.join("a.txt");
        write_text(&file, "hello");
        let scanned_at = now_unix_secs();

        thread::sleep(Duration::from_secs(1));
        write_text(&file, "updated");

        assert!(!is_snapshot_fresh(&dir, scanned_at));

        fs::remove_dir_all(&dir).expect("cleanup temp directory");
    }

    #[test]
    fn snapshot_is_stale_after_file_deletion() {
        let dir = make_temp_dir("delete");
        let file = dir.join("a.txt");
        write_text(&file, "hello");
        let scanned_at = now_unix_secs();

        thread::sleep(Duration::from_secs(1));
        fs::remove_file(&file).expect("remove file");

        assert!(!is_snapshot_fresh(&dir, scanned_at));

        fs::remove_dir_all(&dir).expect("cleanup temp directory");
    }

    #[test]
    fn stale_cache_entry_is_removed_on_read() {
        let dir = make_temp_dir("entry");
        let child_file = dir.join("child.txt");
        write_text(&child_file, "hello");

        let key = normalize_key(&dir);
        let mut store = CacheStore {
            path: dir.join("cache.json"),
            data: CacheFile {
                version: 1,
                dirs: HashMap::from([(
                    key.clone(),
                    CachedDirectory {
                        scanned_at_unix_secs: now_unix_secs(),
                        children: vec![CachedNode {
                            path: child_file.to_string_lossy().into_owned(),
                            size: 5,
                            kind: CachedNodeKind::File,
                        }],
                        size: 5,
                    },
                )]),
            },
        };

        thread::sleep(Duration::from_secs(1));
        write_text(&child_file, "hello world");

        let state = store.get_fresh_directory_state(&dir);
        assert!(state.is_none());
        assert!(!store.data.dirs.contains_key(&key));

        fs::remove_dir_all(&dir).expect("cleanup temp directory");
    }
}
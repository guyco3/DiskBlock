use crate::types::{Node, NodeKind, ScanEvent, ScanResult};
use crossbeam_channel::{unbounded, Receiver, Sender};
use notify::Watcher;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread;

#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub path: PathBuf,
    pub use_sudo: bool,
}

#[derive(Debug, Clone)]
pub struct ScannerHandle {
    tx: Sender<ScanRequest>,
    pub rx: Receiver<ScanEvent>,
    watch_tx: Sender<PathBuf>,
}

impl ScannerHandle {
    pub fn request(&self, path: PathBuf, use_sudo: bool) {
        let _ = self.tx.send(ScanRequest { path, use_sudo });
    }

    pub fn watch(&self, path: PathBuf) {
        let _ = self.watch_tx.send(path);
    }
}

pub fn spawn_scanner() -> ScannerHandle {
    let (req_tx, req_rx) = unbounded::<ScanRequest>();
    let (evt_tx, evt_rx) = unbounded::<ScanEvent>();
    let (watch_tx, watch_rx) = unbounded::<PathBuf>();

    // Scanner thread for directory scanning
    let scanner_evt_tx = evt_tx.clone();
    thread::spawn(move || {
        while let Ok(req) = req_rx.recv() {
            let event = match scan_children(&req.path, req.use_sudo, &scanner_evt_tx) {
                Ok(result) => ScanEvent::Loaded(result),
                Err(err) => ScanEvent::Error {
                    path: req.path.clone(),
                    error: err,
                },
            };
            let _ = scanner_evt_tx.send(event);
        }
    });

    // Watcher thread for filesystem change detection
    let watcher_evt_tx = evt_tx.clone();
    thread::spawn(move || {
        let mut watcher: Option<Box<dyn Watcher>> = None;
        let watched_paths: std::sync::Arc<Mutex<std::collections::HashSet<PathBuf>>> =
            std::sync::Arc::new(Mutex::new(std::collections::HashSet::new()));

        while let Ok(path) = watch_rx.recv() {
            // Initialize watcher on first watch request
            if watcher.is_none() {
                let evt_tx_clone = watcher_evt_tx.clone();
                let watched_clone = watched_paths.clone();

                match notify::recommended_watcher(move |res| {
                    match res {
                        Ok(notify::Event {
                            kind: notify::event::EventKind::Modify(_) | notify::event::EventKind::Remove(_) | notify::event::EventKind::Create(_),
                            paths,
                            ..
                        }) => {
                            if let Ok(watched) = watched_clone.lock() {
                                // Find which watched directory contains this event
                                for watch_dir in watched.iter() {
                                    for changed_path in &paths {
                                        if changed_path.starts_with(watch_dir) {
                                            let _ = evt_tx_clone.send(ScanEvent::CacheInvalidate {
                                                path: watch_dir.clone(),
                                            });
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }) {
                    Ok(w) => watcher = Some(Box::new(w)),
                    Err(_) => continue,
                }
            }

            // Add path to watched set and watch it
            if let Some(ref mut w) = watcher {
                if path.is_dir() {
                    let _ = w.watch(&path, notify::RecursiveMode::NonRecursive);
                    if let Ok(mut watched) = watched_paths.lock() {
                        watched.insert(path);
                    }
                }
            }
        }
    });

    ScannerHandle { tx: req_tx, rx: evt_rx, watch_tx }
}

pub fn can_read_dir(path: &Path) -> bool {
    fs::read_dir(path).is_ok()
}

pub fn disk_total_bytes(path: &Path) -> Option<u64> {
    let output = Command::new("df")
        .arg("-k")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let mut fields = line.split_whitespace();
    let _fs = fields.next()?;
    let blocks = fields.next()?.parse::<u64>().ok()?;
    Some(blocks.saturating_mul(1024))
}

fn scan_children(path: &Path, use_sudo: bool, evt_tx: &Sender<ScanEvent>) -> Result<ScanResult, String> {
    let mut children = if use_sudo {
        scan_children_with_sudo(path, evt_tx)?
    } else {
        scan_children_normal(path, evt_tx)?
    };

    children.sort_by(|a, b| b.size.cmp(&a.size));
    let size = children.iter().map(|n| n.size).sum();

    Ok(ScanResult {
        path: path.to_path_buf(),
        children,
        size,
    })
}

fn scan_children_normal(path: &Path, evt_tx: &Sender<ScanEvent>) -> Result<Vec<Node>, String> {
    let entries = fs::read_dir(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?
        .filter_map(|e| e.ok())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();

    let (node_tx, node_rx) = unbounded::<Node>();
    let producer_entries = entries;

    let producer = thread::spawn(move || {
        producer_entries
            .par_iter()
            .filter_map(|child| {
                let meta = fs::symlink_metadata(child).ok()?;
                let kind = if meta.is_dir() {
                    NodeKind::Directory
                } else {
                    NodeKind::File
                };
                let size = du_size(child).unwrap_or(meta.len());

                Some(Node {
                    path: child.clone(),
                    size,
                    kind,
                    children: None,
                })
            })
            .for_each(|node| {
                let _ = node_tx.send(node);
            });
    });

    let mut nodes = Vec::new();
    for node in node_rx {
        let _ = evt_tx.send(ScanEvent::Partial {
            path: path.to_path_buf(),
            node: node.clone(),
        });
        nodes.push(node);
    }

    let _ = producer.join();

    Ok(nodes)
}

fn scan_children_with_sudo(path: &Path, evt_tx: &Sender<ScanEvent>) -> Result<Vec<Node>, String> {
    let output = Command::new("sudo")
        .arg("-n")
        .arg("find")
        .arg(path)
        .arg("-mindepth")
        .arg("1")
        .arg("-maxdepth")
        .arg("1")
        .arg("-print0")
        .output()
        .map_err(|e| format!("failed to invoke sudo/find: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "sudo required or denied for {}",
            path.display()
        ));
    }

    let mut nodes = Vec::new();
    for raw in output.stdout.split(|b| *b == 0) {
        if raw.is_empty() {
            continue;
        }
        let child = PathBuf::from(String::from_utf8_lossy(raw).to_string());
        let size = du_size_sudo(&child).unwrap_or(0);
        let kind = if is_dir_sudo(&child) {
            NodeKind::Directory
        } else {
            NodeKind::File
        };
        let node = Node {
            path: child.clone(),
            size,
            kind,
            children: None,
        };
        let _ = evt_tx.send(ScanEvent::Partial {
            path: path.to_path_buf(),
            node: node.clone(),
        });
        nodes.push(node);
    }

    Ok(nodes)
}

fn du_size_sudo(path: &Path) -> Option<u64> {
    let output = Command::new("sudo")
        .arg("-n")
        .arg("du")
        .arg("-sk")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let kb = text.split_whitespace().next()?.parse::<u64>().ok()?;
    Some(kb * 1024)
}

fn du_size(path: &Path) -> Option<u64> {
    let output = Command::new("du")
        .arg("-sk")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let kb = text.split_whitespace().next()?.parse::<u64>().ok()?;
    Some(kb * 1024)
}

fn is_dir_sudo(path: &Path) -> bool {
    Command::new("sudo")
        .arg("-n")
        .arg("test")
        .arg("-d")
        .arg(path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}


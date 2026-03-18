use std::path::PathBuf;

/// Classifies a filesystem node as directory or file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Directory,
    File,
}

/// A scanned filesystem entry and its measured size.
#[derive(Debug, Clone)]
pub struct Node {
    pub path: PathBuf,
    pub size: u64,
    pub kind: NodeKind,
}

/// Final scan payload for a single directory.
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub path: PathBuf,
    pub children: Vec<Node>,
    pub size: u64,
}

/// Events emitted by scanner threads.
#[derive(Debug, Clone)]
pub enum ScanEvent {
    Partial { path: PathBuf, node: Node },
    Loaded(ScanResult),
    Error { path: PathBuf, error: String },
    PermissionRequired { path: PathBuf },
    CacheInvalidate { path: PathBuf },
}

/// A concrete rectangle rendered in the treemap area.
#[derive(Debug, Clone)]
pub struct RectNode {
    pub path: PathBuf,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub size: u64,
    pub label: String,
    pub is_dir: bool,
}

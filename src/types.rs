use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Directory,
    File,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub path: PathBuf,
    pub size: u64,
    pub kind: NodeKind,
    pub children: Option<Vec<Node>>, // lazily loaded
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub path: PathBuf,
    pub children: Vec<Node>,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum ScanEvent {
    Partial { path: PathBuf, node: Node },
    Loaded(ScanResult),
    Error { path: PathBuf, error: String },
    CacheInvalidate { path: PathBuf }, // notify that a cached dir changed on disk
}

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
    pub is_other: bool,
    pub other_items: Vec<String>,
}

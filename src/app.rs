use crate::format::human_size;
use crate::cache::CacheStore;
use crate::scanner::ScannerHandle;
use crate::types::{Node, NodeKind, RectNode, ScanEvent};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Direction for geometric sibling navigation.
#[derive(Debug, Clone, Copy)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Cached scanning and rendering state for a directory.
#[derive(Debug, Clone)]
pub struct DirectoryState {
    pub children: Vec<Node>,
    pub size: u64,
    pub loaded_size: u64,
    pub size_locked: bool,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
}

/// Core application state used by input handling and rendering.
#[derive(Debug, Clone)]
pub struct App {
    pub current_path: PathBuf,
    pub root_path: PathBuf,
    pub scanner: ScannerHandle,
    pub dirs: HashMap<PathBuf, DirectoryState>,
    pub selected_idx: usize,
    pub status: String,
    pub root_size: u64,
    pub disk_total: u64,
    sudo_paths: HashSet<PathBuf>,
    cache: CacheStore,
    pub show_help: bool,
}

impl App {
    /// Creates a new app rooted at `root_path` and starts the initial scan.
    pub fn new(root_path: PathBuf, scanner: ScannerHandle) -> Self {
        let mut cache = CacheStore::load_default();
        let mut dirs = HashMap::new();
        let root_cached = cache.get_fresh_directory_state(&root_path);

        dirs.insert(
            root_path.clone(),
            root_cached.unwrap_or(DirectoryState {
                children: Vec::new(),
                size: 0,
                loaded_size: 0,
                size_locked: false,
                loading: true,
                loaded: false,
                error: None,
            }),
        );

        let status = if dirs
            .get(&root_path)
            .map(|s| s.loaded && !s.children.is_empty())
            .unwrap_or(false)
        {
            format!("Loaded from cache {}", root_path.display())
        } else {
            "Scanning...".to_string()
        };

        let app = Self {
            current_path: root_path.clone(),
            root_path: root_path.clone(),
            scanner,
            dirs,
            selected_idx: 0,
            status,
            root_size: 0,
            disk_total: crate::scanner::disk_total_bytes(&root_path).unwrap_or(0),
            sudo_paths: HashSet::new(),
            cache,
            show_help: false,
        };

        let mut app = app;
        app.root_size = app
            .dirs
            .get(&root_path)
            .map(|s| s.size)
            .unwrap_or(0);

        app.scanner.watch(root_path.clone());
        if !app
            .dirs
            .get(&root_path)
            .map(|s| s.loaded)
            .unwrap_or(false)
        {
            app.scanner.request(root_path, false);
        }
        app
    }

    pub fn current_state(&self) -> Option<&DirectoryState> {
        self.dirs.get(&self.current_path)
    }

    pub fn current_render_nodes(&self) -> Option<Vec<Node>> {
        let state = self.current_state()?;
        let mut render_nodes = state.children.clone();
        if state.loading && state.size > state.loaded_size {
            render_nodes.push(Node {
                path: self.current_path.join("<Loading>"),
                size: state.size - state.loaded_size,
                kind: NodeKind::File,
            });
        }
        Some(render_nodes)
    }

    pub fn selected_rendered_rect(&self, bounds: crate::layout::Bounds) -> Option<RectNode> {
        let render_nodes = self.current_render_nodes()?;

        let rects = crate::layout::compute_partition_oriented(
            &self.current_path,
            &render_nodes,
            bounds,
            true,
        );
        if rects.is_empty() {
            return None;
        }
        let idx = self.selected_idx.min(rects.len() - 1);
        rects.get(idx).cloned()
    }

    pub fn selected_rendered_node(&self, bounds: crate::layout::Bounds) -> Option<Node> {
        let rect = self.selected_rendered_rect(bounds)?;
        let render_nodes = self.current_render_nodes()?;
        render_nodes.into_iter().find(|n| n.path == rect.path)
    }

    fn current_display_len(&self, _bounds: crate::layout::Bounds) -> usize {
        self.current_render_nodes().map_or(0, |nodes| nodes.len())
    }

    pub fn move_next(&mut self, bounds: crate::layout::Bounds) {
        let len = self.current_display_len(bounds);
        if len == 0 {
            return;
        }
        self.selected_idx = (self.selected_idx + 1).min(len.saturating_sub(1));
    }

    pub fn move_prev(&mut self) {
        self.selected_idx = self.selected_idx.saturating_sub(1);
    }

    pub fn move_geometric(&mut self, direction: NavDirection, bounds: crate::layout::Bounds) {
        let Some(render_nodes) = self.current_render_nodes() else {
            return;
        };

        let rects = crate::layout::compute_partition_oriented(
            &self.current_path,
            &render_nodes,
            bounds,
            true,
        );
        if rects.is_empty() {
            return;
        }

        if self.selected_idx >= rects.len() {
            self.selected_idx = rects.len() - 1;
        }

        let current = &rects[self.selected_idx];
        let cur_x = i32::from(current.x);
        let cur_y = i32::from(current.y);

        let mut by_yx: Vec<usize> = (0..rects.len()).collect();
        by_yx.sort_by_key(|&i| (rects[i].y, rects[i].x));

        let mut best_idx: Option<usize> = None;
        let mut best_score = (u32::MAX, u32::MAX, u32::MAX);

        for (idx, rect) in rects.iter().enumerate() {
            if idx == self.selected_idx {
                continue;
            }

            let tx = i32::from(rect.x);
            let ty = i32::from(rect.y);

            let candidate = match direction {
                NavDirection::Right if tx > cur_x => {
                    Some(((ty - cur_y).unsigned_abs(), (tx - cur_x) as u32, tx.unsigned_abs()))
                }
                NavDirection::Left if tx < cur_x => {
                    Some(((ty - cur_y).unsigned_abs(), (cur_x - tx) as u32, tx.unsigned_abs()))
                }
                NavDirection::Down if ty > cur_y => {
                    Some(((ty - cur_y) as u32, (tx - cur_x).unsigned_abs(), tx.unsigned_abs()))
                }
                NavDirection::Up if ty < cur_y => {
                    Some(((cur_y - ty) as u32, (tx - cur_x).unsigned_abs(), tx.unsigned_abs()))
                }
                _ => None,
            };

            if let Some(score) = candidate {
                if score < best_score {
                    best_score = score;
                    best_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = best_idx {
            self.selected_idx = idx;
            return;
        }

        if let Some(pos) = by_yx.iter().position(|&i| i == self.selected_idx) {
            match direction {
                NavDirection::Down | NavDirection::Right => {
                    if let Some(&next) = by_yx.get(pos + 1) {
                        self.selected_idx = next;
                    }
                }
                NavDirection::Up | NavDirection::Left => {
                    if pos > 0 {
                        self.selected_idx = by_yx[pos - 1];
                    }
                }
            }
        }
    }

    pub fn enter_node(&mut self, selected: Node, use_sudo: bool) {
        if selected.kind != NodeKind::Directory {
            return;
        }

        if use_sudo {
            self.sudo_paths.insert(selected.path.clone());
        }

        self.current_path = selected.path.clone();
        self.selected_idx = 0;

        if !self.dirs.contains_key(&selected.path) {
            if let Some(state) = self.cache.get_fresh_directory_state(&selected.path) {
                self.dirs.insert(selected.path.clone(), state);
                self.status = format!("Loaded from cache {}", selected.path.display());
            } else {
                self.dirs.insert(
                    selected.path.clone(),
                    DirectoryState {
                        children: Vec::new(),
                        size: selected.size,
                        loaded_size: 0,
                        size_locked: selected.size > 0,
                        loading: true,
                        loaded: false,
                        error: None,
                    },
                );
                let scan_with_sudo = self.sudo_paths.contains(&selected.path);
                self.scanner.request(selected.path.clone(), scan_with_sudo);
                self.status = format!("Scanning {}", selected.path.display());
            }
        }

        self.scanner.watch(selected.path.clone());
    }

    pub fn go_parent(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            // Stop only when we reach the filesystem root (parent == current on Unix)
            if parent == self.current_path {
                return;
            }
            
            let parent_path = parent.to_path_buf();
            
            // Request scan if this path hasn't been loaded yet
            if !self.dirs.contains_key(&parent_path) {
                if let Some(state) = self.cache.get_fresh_directory_state(&parent_path) {
                    self.dirs.insert(parent_path.clone(), state);
                    self.status = format!("Loaded from cache {}", parent_path.display());
                } else {
                    self.dirs.insert(
                        parent_path.clone(),
                        DirectoryState {
                            children: Vec::new(),
                            size: 0,
                            loaded_size: 0,
                            size_locked: false,
                            loading: true,
                            loaded: false,
                            error: None,
                        },
                    );
                    let scan_with_sudo = self.sudo_paths.contains(&parent_path);
                    self.scanner.request(parent_path.clone(), scan_with_sudo);
                    self.status = format!("Scanning {}", parent_path.display());
                }
            } else {
                self.status = format!("Viewing {}", parent_path.display());
            }
            
            self.current_path = parent_path.clone();
            self.selected_idx = 0;
            self.scanner.watch(parent_path);
        }
    }

    pub fn on_scan_event(&mut self, event: ScanEvent) {
        match event {
            ScanEvent::Loaded(result) => {
                let state = self.dirs.entry(result.path.clone()).or_insert(DirectoryState {
                    children: Vec::new(),
                    size: 0,
                    loaded_size: 0,
                    size_locked: false,
                    loading: false,
                    loaded: false,
                    error: None,
                });
                if state.children.is_empty() {
                    state.children = result.children;
                }
                state.loaded_size = result.size;
                if !state.size_locked {
                    state.size = result.size;
                    state.size_locked = true;
                }
                state.loading = false;
                state.loaded = true;
                state.error = None;
                self.cache.put_directory_state(&result.path, state);

                if result.path == self.root_path {
                    self.root_size = state.size;
                }

                if result.path == self.current_path {
                    let count = state.children.len();
                    if count > 0 {
                        self.selected_idx = self.selected_idx.min(count - 1);
                    } else {
                        self.selected_idx = 0;
                    }
                }

                self.status = format!("Loaded {}", result.path.display());
            }
            ScanEvent::Partial { path, node } => {
                let item_path = node.path.clone();
                let item_kind = node.kind;
                let state = self.dirs.entry(path.clone()).or_insert(DirectoryState {
                    children: Vec::new(),
                    size: 0,
                    loaded_size: 0,
                    size_locked: false,
                    loading: true,
                    loaded: false,
                    error: None,
                });
                state.loading = true;
                state.children.push(node);
                state.loaded_size = state.children.iter().map(|n| n.size).sum();
                if !state.size_locked {
                    state.size = state.loaded_size;
                }
                if path == self.root_path {
                    self.root_size = state.size;
                }

                let item_name = item_path.file_name().and_then(|n| n.to_str()).unwrap_or("/");
                let kind_prefix = if item_kind == NodeKind::Directory {
                    "dir"
                } else {
                    "file"
                };
                self.status = format!(
                    "Scanning {} | current {}: {} | discovered {} items | {}",
                    path.display(),
                    kind_prefix,
                    item_name,
                    state.children.len(),
                    human_size(state.loaded_size)
                );
            }
            ScanEvent::Error { path, error } => {
                let state = self.dirs.entry(path.clone()).or_insert(DirectoryState {
                    children: Vec::new(),
                    size: 0,
                    loaded_size: 0,
                    size_locked: false,
                    loading: false,
                    loaded: false,
                    error: None,
                });
                state.loading = false;
                state.error = Some(error.clone());
                self.status = error;
            }
            ScanEvent::PermissionRequired { path } => {
                self.status = format!("Permission required for {}", path.display());
            }
            ScanEvent::CacheInvalidate { path } => {
                if let Some(state) = self.dirs.get_mut(&path) {
                    state.children.clear();
                    state.loaded_size = 0;
                    state.size_locked = false;
                    state.loading = true;
                    state.loaded = false;
                    state.error = None;

                    let scan_with_sudo = self.sudo_paths.contains(&path);
                    self.scanner.request(path.clone(), scan_with_sudo);
                    if path == self.current_path {
                        self.status = format!("Refreshing {}", path.display());
                        self.selected_idx = 0;
                    }
                }
            }
        }
    }

    pub fn persist_cache(&self) -> Result<(), String> {
        self.cache.save()
    }

    pub fn rescan_with_sudo(&mut self, path: PathBuf) {
        self.sudo_paths.insert(path.clone());

        let state = self.dirs.entry(path.clone()).or_insert(DirectoryState {
            children: Vec::new(),
            size: 0,
            loaded_size: 0,
            size_locked: false,
            loading: true,
            loaded: false,
            error: None,
        });
        state.children.clear();
        state.loaded_size = 0;
        state.size_locked = false;
        state.loading = true;
        state.loaded = false;
        state.error = None;

        self.scanner.request(path.clone(), true);
        self.scanner.watch(path.clone());
        self.status = format!("Rescanning with sudo {}", path.display());
    }

    pub fn breadcrumbs(&self) -> Vec<PathBuf> {
        let mut parts = Vec::new();
        let mut cur: &Path = &self.current_path;
        loop {
            parts.push(cur.to_path_buf());
            if let Some(parent) = cur.parent() {
                if parent == cur {
                    break;
                }
                cur = parent;
            } else {
                break;
            }
        }
        parts.reverse();
        parts
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }
}

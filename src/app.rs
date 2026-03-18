use crate::scanner::ScannerHandle;
use crate::types::{Node, NodeKind, RectNode, ScanEvent};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

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
    pub show_help: bool,
    viewing_other_parent: Option<PathBuf>, // if Some(parent_path), we're viewing an Other directory
    other_items: Vec<Node>,                 // items being displayed in Other view
}

impl App {
    pub fn new(root_path: PathBuf, scanner: ScannerHandle) -> Self {
        let mut dirs = HashMap::new();
        dirs.insert(
            root_path.clone(),
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

        let app = Self {
            current_path: root_path.clone(),
            root_path: root_path.clone(),
            scanner,
            dirs,
            selected_idx: 0,
            status: "Scanning...".to_string(),
            root_size: 0,
            disk_total: crate::scanner::disk_total_bytes(&root_path).unwrap_or(0),
            sudo_paths: HashSet::new(),
            show_help: false,
            viewing_other_parent: None,
            other_items: Vec::new(),
        };

        app.scanner.request(root_path, false);
        app
    }

    pub fn current_state(&self) -> Option<&DirectoryState> {
        self.dirs.get(&self.current_path)
    }

    pub fn selected_child(&self) -> Option<&Node> {
        self.current_state()?.children.get(self.selected_idx)
    }

    pub fn is_viewing_other(&self) -> bool {
        self.viewing_other_parent.is_some()
    }

    pub fn current_render_nodes(&self) -> Option<Vec<Node>> {
        // If viewing Other directory, return those items sorted by size
        if self.viewing_other_parent.is_some() {
            let mut items = self.other_items.clone();
            items.sort_by(|a, b| b.size.cmp(&a.size));
            return Some(items);
        }

        let state = self.current_state()?;
        let mut render_nodes = state.children.clone();
        if state.loading && state.size > state.loaded_size {
            render_nodes.push(Node {
                path: self.current_path.join("<Loading>"),
                size: state.size - state.loaded_size,
                kind: NodeKind::File,
                children: None,
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
        let display_nodes = crate::layout::build_display_nodes(&self.current_path, &render_nodes);
        display_nodes.into_iter().find(|n| n.path == rect.path)
    }

    fn current_display_len(&self) -> usize {
        let Some(render_nodes) = self.current_render_nodes() else {
            return 0;
        };
        crate::layout::build_display_nodes(&self.current_path, &render_nodes).len()
    }

    pub fn move_next(&mut self) {
        let len = self.current_display_len();
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
        let cx = center_x(current.x, current.width);
        let cy = center_y(current.y, current.height);

        let mut best_idx: Option<usize> = None;
        let mut best_score = (u32::MAX, u32::MAX, u32::MAX);

        for (idx, rect) in rects.iter().enumerate() {
            if idx == self.selected_idx {
                continue;
            }

            let tx = center_x(rect.x, rect.width);
            let ty = center_y(rect.y, rect.height);
            let dx = tx - cx;
            let dy = ty - cy;

            let valid = match direction {
                NavDirection::Left => dx < 0,
                NavDirection::Right => dx > 0,
                NavDirection::Up => dy < 0,
                NavDirection::Down => dy > 0,
            };
            if !valid {
                continue;
            }

            let primary = match direction {
                NavDirection::Left | NavDirection::Right => dx.unsigned_abs(),
                NavDirection::Up | NavDirection::Down => dy.unsigned_abs(),
            };
            let secondary = match direction {
                NavDirection::Left | NavDirection::Right => dy.unsigned_abs(),
                NavDirection::Up | NavDirection::Down => dx.unsigned_abs(),
            };
            let euclid_sq = dx.unsigned_abs().saturating_mul(dx.unsigned_abs())
                + dy.unsigned_abs().saturating_mul(dy.unsigned_abs());
            let score = (primary, secondary, euclid_sq);

            if score < best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            self.selected_idx = idx;
        }
    }

    pub fn can_enter_selected(&self) -> bool {
        self.selected_child()
            .map(|n| n.kind == NodeKind::Directory)
            .unwrap_or(false)
    }

    pub fn enter_node(&mut self, selected: Node, use_sudo: bool) {
        if selected.kind != NodeKind::Directory {
            return;
        }

        // Check if this is an <Other> virtual directory
        if selected.path.file_name().and_then(|n| n.to_str()) == Some("<Other>") {
            if let Some(other_children) = selected.children {
                self.viewing_other_parent = Some(self.current_path.clone());
                self.current_path = self.current_path.join("<Other>");
                self.other_items = other_children;
                self.selected_idx = 0;
                self.status = format!("Viewing {} items", self.other_items.len());
            }
            return;
        }

        // Exit Other view if we're entering a real directory
        self.viewing_other_parent = None;
        self.other_items.clear();

        if use_sudo {
            self.sudo_paths.insert(selected.path.clone());
        }

        self.current_path = selected.path.clone();
        self.selected_idx = 0;

        if !self.dirs.contains_key(&selected.path) {
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

        // Watch the directory for filesystem changes
        self.scanner.watch(selected.path.clone());
    }

    pub fn go_parent(&mut self) {
        // If viewing Other, exit to the parent directory we were in
        if let Some(parent) = self.viewing_other_parent.take() {
            self.current_path = parent.clone();
            self.other_items.clear();
            self.selected_idx = 0;
            self.status = format!("Viewing {}", self.current_path.display());
            return;
        }

        if self.current_path == self.root_path {
            return;
        }
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.selected_idx = 0;
            self.status = format!("Viewing {}", self.current_path.display());
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
                self.status = format!("{}", error);
            }
            ScanEvent::CacheInvalidate { path } => {
                // Clear the cache for this directory and re-scan it
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

fn center_x(x: u16, w: u16) -> i32 {
    i32::from(x) + i32::from(w) / 2
}

fn center_y(y: u16, h: u16) -> i32 {
    i32::from(y) + i32::from(h) / 2
}

use crate::types::{Node, RectNode};
use std::path::Path;

const CUMULATIVE_PERCENT_TARGET: f64 = 80.0;
const MAX_VISIBLE_NODES: usize = 20;

#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

pub fn compute_partition(root_path: &Path, children: &[Node], bounds: Bounds) -> Vec<RectNode> {
    compute_partition_oriented(root_path, children, bounds, true)
}

pub fn build_display_nodes(root_path: &Path, children: &[Node]) -> Vec<Node> {
    let mut sorted = children.to_vec();
    sorted.sort_by(|a, b| b.size.cmp(&a.size));

    let total = sorted.iter().map(|n| n.size).sum::<u64>().max(1);
    let mut kept = Vec::new();
    let mut other = Vec::new();

    // Keep showing items until cumulative size reaches CUMULATIVE_PERCENT_TARGET.
    let target_size = (total as f64 * (CUMULATIVE_PERCENT_TARGET / 100.0)) as u64;
    let mut cumulative = 0u64;

    for node in sorted {
        if cumulative >= target_size || kept.len() >= MAX_VISIBLE_NODES {
            other.push(node);
        } else {
            cumulative += node.size;
            kept.push(node);
        }
    }

    let other_size = other.iter().map(|n| n.size).sum::<u64>();
    if other_size > 0 {
        kept.push(Node {
            path: root_path.join("<Other>"),
            size: other_size,
            kind: crate::types::NodeKind::Directory,
            children: Some(other),
        });
    }

    kept
}

pub fn compute_partition_oriented(
    root_path: &Path,
    children: &[Node],
    bounds: Bounds,
    _horizontal: bool,
) -> Vec<RectNode> {
    let kept = build_display_nodes(root_path, children);
    partition_level(&kept, bounds)
}

fn partition_level(children: &[Node], bounds: Bounds) -> Vec<RectNode> {
    let mut out = Vec::new();
    if children.is_empty() || bounds.width == 0 || bounds.height == 0 {
        return out;
    }

    let total_size = children.iter().map(|n| n.size).sum::<u64>();
    if total_size == 0 {
        return out;
    }

    let total_area = f64::from(bounds.width) * f64::from(bounds.height);
    let mut items: Vec<(&Node, f64)> = children
        .iter()
        .map(|n| (n, (n.size as f64 / total_size as f64) * total_area))
        .collect();

    items.sort_by(|a, b| b.0.size.cmp(&a.0.size));

    let mut remaining = bounds;
    let mut cursor = 0usize;

    while cursor < items.len() && remaining.width > 0 && remaining.height > 0 {
        let horizontal = remaining.width >= remaining.height;
        let span = if horizontal {
            f64::from(remaining.width)
        } else {
            f64::from(remaining.height)
        };
        if span <= 0.0 {
            break;
        }

        let mut row_end = cursor + 1;
        let mut row_areas = vec![items[cursor].1.max(0.0001)];
        while row_end < items.len() {
            let mut candidate = row_areas.clone();
            candidate.push(items[row_end].1.max(0.0001));
            if worst_aspect_ratio(&candidate, span) <= worst_aspect_ratio(&row_areas, span) {
                row_areas = candidate;
                row_end += 1;
            } else {
                break;
            }
        }

        let row_nodes = &items[cursor..row_end];
        let row_total_size = row_nodes.iter().map(|(n, _)| n.size).sum::<u64>();
        if row_total_size == 0 {
            cursor = row_end;
            continue;
        }

        let is_last_row = row_end >= items.len();
        if horizontal {
            let mut row_h = if is_last_row {
                remaining.height
            } else {
                let h = (row_areas.iter().sum::<f64>() / span).round() as i32;
                h.clamp(1, i32::from(remaining.height)) as u16
            };
            if row_h == 0 {
                row_h = 1;
            }

            let mut x = remaining.x;
            for (idx, (child, _)) in row_nodes.iter().enumerate() {
                let is_last = idx == row_nodes.len() - 1;
                let w = if is_last {
                    remaining.x + remaining.width - x
                } else {
                    proportional_len(child.size, row_total_size, remaining.width)
                };
                if w == 0 {
                    continue;
                }
                out.push(make_rect_node(child, x, remaining.y, w, row_h));
                x = x.saturating_add(w);
            }

            remaining.y = remaining.y.saturating_add(row_h);
            remaining.height = remaining.height.saturating_sub(row_h);
        } else {
            let mut row_w = if is_last_row {
                remaining.width
            } else {
                let w = (row_areas.iter().sum::<f64>() / span).round() as i32;
                w.clamp(1, i32::from(remaining.width)) as u16
            };
            if row_w == 0 {
                row_w = 1;
            }

            let mut y = remaining.y;
            for (idx, (child, _)) in row_nodes.iter().enumerate() {
                let is_last = idx == row_nodes.len() - 1;
                let h = if is_last {
                    remaining.y + remaining.height - y
                } else {
                    proportional_len(child.size, row_total_size, remaining.height)
                };
                if h == 0 {
                    continue;
                }
                out.push(make_rect_node(child, remaining.x, y, row_w, h));
                y = y.saturating_add(h);
            }

            remaining.x = remaining.x.saturating_add(row_w);
            remaining.width = remaining.width.saturating_sub(row_w);
        }

        cursor = row_end;
    }

    out
}

fn worst_aspect_ratio(areas: &[f64], span: f64) -> f64 {
    if areas.is_empty() || span <= 0.0 {
        return f64::INFINITY;
    }
    let sum = areas.iter().sum::<f64>();
    if sum <= 0.0 {
        return f64::INFINITY;
    }
    let min_area = areas.iter().copied().fold(f64::INFINITY, f64::min);
    let max_area = areas.iter().copied().fold(0.0, f64::max);
    if min_area <= 0.0 {
        return f64::INFINITY;
    }

    let span_sq = span * span;
    let sum_sq = sum * sum;
    let a = (span_sq * max_area) / sum_sq;
    let b = sum_sq / (span_sq * min_area);
    a.max(b)
}

fn make_rect_node(child: &Node, x: u16, y: u16, width: u16, height: u16) -> RectNode {
    let label = child
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("/")
        .to_string();

    RectNode {
        path: child.path.clone(),
        x,
        y,
        width,
        height,
        size: child.size,
        label,
        is_dir: matches!(child.kind, crate::types::NodeKind::Directory),
        is_other: child
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "<Other>")
            .unwrap_or(false),
        other_items: child
            .children
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|n| n.path.file_name().and_then(|x| x.to_str()).map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    }
}

fn proportional_len(size: u64, total: u64, span: u16) -> u16 {
    if total == 0 || span == 0 {
        return 0;
    }
    let raw = (size as u128 * span as u128) / total as u128;
    raw as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Node, NodeKind};
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn partitions_fill_area() {
        let nodes = vec![
            Node { path: PathBuf::from("a"), size: 60, kind: NodeKind::Directory, children: None },
            Node { path: PathBuf::from("b"), size: 30, kind: NodeKind::Directory, children: None },
            Node { path: PathBuf::from("c"), size: 10, kind: NodeKind::Directory, children: None },
        ];
        let bounds = Bounds { x: 0, y: 0, width: 100, height: 20 };
        let rects = compute_partition(Path::new("/"), &nodes, bounds);
        let sum_area: u32 = rects.iter().map(area).sum();
        assert_eq!(sum_area, u32::from(bounds.width) * u32::from(bounds.height));
    }

    #[test]
    fn tiny_items_are_grouped_into_other() {
        let mut nodes = Vec::new();
        nodes.push(Node { path: PathBuf::from("big"), size: 10_000, kind: NodeKind::Directory, children: None });
        for i in 0..50 {
            nodes.push(Node {
                path: PathBuf::from(format!("small-{i}.txt")),
                size: 10,
                kind: NodeKind::File,
                children: None,
            });
        }

        let bounds = Bounds { x: 0, y: 0, width: 120, height: 30 };
        let rects = compute_partition(Path::new("/"), &nodes, bounds);
        assert!(rects.iter().any(|r| r.is_other));
    }

    #[test]
    fn partitions_have_no_overlap() {
        let nodes = vec![
            Node { path: PathBuf::from("a"), size: 70, kind: NodeKind::Directory, children: None },
            Node { path: PathBuf::from("b"), size: 20, kind: NodeKind::Directory, children: None },
            Node { path: PathBuf::from("c"), size: 10, kind: NodeKind::Directory, children: None },
        ];
        let bounds = Bounds { x: 0, y: 0, width: 120, height: 40 };
        let rects = compute_partition(Path::new("/"), &nodes, bounds);

        let mut total_area: u32 = 0;
        for rect in &rects {
            total_area = total_area.saturating_add(area(rect));
        }
        assert_eq!(total_area, u32::from(bounds.width) * u32::from(bounds.height));

        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(!overlaps(&rects[i], &rects[j]));
            }
        }
    }

    #[test]
    fn partitions_are_proportional_with_tolerance() {
        let nodes = vec![
            Node { path: PathBuf::from("a"), size: 500, kind: NodeKind::Directory, children: None },
            Node { path: PathBuf::from("b"), size: 300, kind: NodeKind::Directory, children: None },
            Node { path: PathBuf::from("c"), size: 200, kind: NodeKind::Directory, children: None },
        ];
        let bounds = Bounds { x: 0, y: 0, width: 200, height: 60 };
        let rects = compute_partition(Path::new("/"), &nodes, bounds);

        let total_size: u64 = nodes.iter().map(|n| n.size).sum();
        let total_area = f64::from(bounds.width) * f64::from(bounds.height);

        let mut area_by_path: HashMap<PathBuf, u32> = HashMap::new();
        for rect in &rects {
            area_by_path.insert(rect.path.clone(), area(rect));
        }

        // Find which items are directly shown vs grouped into <Other>
        let _other_path = PathBuf::from("<Other>");
        let directly_shown: Vec<_> = nodes.iter()
            .filter(|n| area_by_path.contains_key(&n.path))
            .collect();

        // Check proportionality only for directly shown items
        for node in directly_shown {
            let actual = f64::from(*area_by_path.get(&node.path).unwrap_or(&0));
            let expected = (node.size as f64 / total_size as f64) * total_area;
            let diff_ratio = if total_area > 0.0 {
                (actual - expected).abs() / total_area
            } else {
                0.0
            };
            assert!(
                diff_ratio <= 0.02,
                "area diff too large for {:?}: actual={actual}, expected={expected}, ratio={diff_ratio}",
                node.path
            );
        }
    }

    fn area(rect: &RectNode) -> u32 {
        u32::from(rect.width) * u32::from(rect.height)
    }

    fn overlaps(a: &RectNode, b: &RectNode) -> bool {
        let ax2 = a.x.saturating_add(a.width);
        let ay2 = a.y.saturating_add(a.height);
        let bx2 = b.x.saturating_add(b.width);
        let by2 = b.y.saturating_add(b.height);

        let x_overlaps = a.x < bx2 && b.x < ax2;
        let y_overlaps = a.y < by2 && b.y < ay2;
        x_overlaps && y_overlaps
    }
}

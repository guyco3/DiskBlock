use crate::types::{Node, RectNode};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

pub fn compute_partition_oriented(
    _root_path: &Path,
    children: &[Node],
    bounds: Bounds,
    _horizontal: bool,
) -> Vec<RectNode> {
    partition_level(children, bounds)
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

    let mut ordered: Vec<&Node> = children.iter().collect();
    ordered.sort_by(|a, b| b.size.cmp(&a.size));
    partition_binary(&mut out, &ordered, bounds);

    out
}

fn partition_binary(out: &mut Vec<RectNode>, nodes: &[&Node], bounds: Bounds) {
    if nodes.is_empty() || bounds.width == 0 || bounds.height == 0 {
        return;
    }

    if nodes.len() == 1 {
        out.push(make_rect_node(nodes[0], bounds.x, bounds.y, bounds.width, bounds.height));
        return;
    }

    let total: u64 = nodes.iter().map(|n| n.size).sum();
    if total == 0 {
        return;
    }

    let mut acc = 0u64;
    let half = total / 2;
    let mut split_idx = 1usize;
    while split_idx < nodes.len() {
        let next = acc + nodes[split_idx - 1].size;
        if next >= half {
            break;
        }
        acc = next;
        split_idx += 1;
    }
    split_idx = split_idx.clamp(1, nodes.len() - 1);
    let left_total = nodes[..split_idx].iter().map(|n| n.size).sum::<u64>();
    let right_total = total.saturating_sub(left_total);

    let prefer_vertical = should_split_vertical(bounds, left_total, total);
    if prefer_vertical {
        let mut left_w = if right_total == 0 {
            bounds.width
        } else {
            proportional_len(left_total, total, bounds.width)
        };
        left_w = left_w.clamp(1, bounds.width.saturating_sub(1).max(1));
        let right_w = bounds.width.saturating_sub(left_w);

        let left_bounds = Bounds {
            x: bounds.x,
            y: bounds.y,
            width: left_w,
            height: bounds.height,
        };
        let right_bounds = Bounds {
            x: bounds.x.saturating_add(left_w),
            y: bounds.y,
            width: right_w,
            height: bounds.height,
        };

        partition_binary(out, &nodes[..split_idx], left_bounds);
        if right_w > 0 {
            partition_binary(out, &nodes[split_idx..], right_bounds);
        }
    } else {
        let mut top_h = if right_total == 0 {
            bounds.height
        } else {
            proportional_len(left_total, total, bounds.height)
        };
        top_h = top_h.clamp(1, bounds.height.saturating_sub(1).max(1));
        let bottom_h = bounds.height.saturating_sub(top_h);

        let top_bounds = Bounds {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: top_h,
        };
        let bottom_bounds = Bounds {
            x: bounds.x,
            y: bounds.y.saturating_add(top_h),
            width: bounds.width,
            height: bottom_h,
        };

        partition_binary(out, &nodes[..split_idx], top_bounds);
        if bottom_h > 0 {
            partition_binary(out, &nodes[split_idx..], bottom_bounds);
        }
    }
}

fn should_split_vertical(bounds: Bounds, left_total: u64, total: u64) -> bool {
    if bounds.width == 0 || bounds.height == 0 || total == 0 {
        return bounds.width >= bounds.height;
    }

    let left_w = proportional_len(left_total, total, bounds.width).clamp(1, bounds.width.max(1));
    let right_w = bounds.width.saturating_sub(left_w).max(1);
    let vert_score = aspect_ratio(left_w, bounds.height).max(aspect_ratio(right_w, bounds.height));

    let top_h = proportional_len(left_total, total, bounds.height).clamp(1, bounds.height.max(1));
    let bottom_h = bounds.height.saturating_sub(top_h).max(1);
    let hori_score = aspect_ratio(bounds.width, top_h).max(aspect_ratio(bounds.width, bottom_h));

    vert_score <= hori_score
}

fn aspect_ratio(w: u16, h: u16) -> f64 {
    let wf = f64::from(w.max(1));
    let hf = f64::from(h.max(1));
    if wf >= hf { wf / hf } else { hf / wf }
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
            Node { path: PathBuf::from("a"), size: 60, kind: NodeKind::Directory },
            Node { path: PathBuf::from("b"), size: 30, kind: NodeKind::Directory },
            Node { path: PathBuf::from("c"), size: 10, kind: NodeKind::Directory },
        ];
        let bounds = Bounds { x: 0, y: 0, width: 100, height: 20 };
        let rects = compute_partition_oriented(Path::new("/"), &nodes, bounds, true);
        let sum_area: u32 = rects.iter().map(area).sum();
        assert_eq!(sum_area, u32::from(bounds.width) * u32::from(bounds.height));
    }

    #[test]
    fn renders_all_items_without_grouping() {
        let mut nodes = Vec::new();
        nodes.push(Node { path: PathBuf::from("big"), size: 10_000, kind: NodeKind::Directory });
        for i in 0..50 {
            nodes.push(Node {
                path: PathBuf::from(format!("small-{i}.txt")),
                size: 10,
                kind: NodeKind::File,
            });
        }

        let bounds = Bounds { x: 0, y: 0, width: 120, height: 30 };
        let rects = compute_partition_oriented(Path::new("/"), &nodes, bounds, true);
        assert_eq!(rects.len(), nodes.len());
    }

    #[test]
    fn partitions_have_no_overlap() {
        let nodes = vec![
            Node { path: PathBuf::from("a"), size: 70, kind: NodeKind::Directory },
            Node { path: PathBuf::from("b"), size: 20, kind: NodeKind::Directory },
            Node { path: PathBuf::from("c"), size: 10, kind: NodeKind::Directory },
        ];
        let bounds = Bounds { x: 0, y: 0, width: 120, height: 40 };
        let rects = compute_partition_oriented(Path::new("/"), &nodes, bounds, true);

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
            Node { path: PathBuf::from("a"), size: 500, kind: NodeKind::Directory },
            Node { path: PathBuf::from("b"), size: 300, kind: NodeKind::Directory },
            Node { path: PathBuf::from("c"), size: 200, kind: NodeKind::Directory },
        ];
        let bounds = Bounds { x: 0, y: 0, width: 200, height: 60 };
        let rects = compute_partition_oriented(Path::new("/"), &nodes, bounds, true);

        let total_size: u64 = nodes.iter().map(|n| n.size).sum();
        let total_area = f64::from(bounds.width) * f64::from(bounds.height);

        let mut area_by_path: HashMap<PathBuf, u32> = HashMap::new();
        for rect in &rects {
            area_by_path.insert(rect.path.clone(), area(rect));
        }

        for node in &nodes {
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

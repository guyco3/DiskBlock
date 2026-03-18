use crate::app::App;
use crate::format::{human_size, pct_of};
use crate::layout::{compute_partition_oriented, Bounds};
use crate::types::RectNode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = split_main_layout(frame.area());
    let selected = app.selected_rendered_rect(main_bounds_from_terminal(frame.area()));

    draw_top_bar(frame, app, chunks[0]);
    draw_main(frame, app, chunks[1]);
    draw_bottom_bar(frame, app, chunks[2], selected.as_ref());

    if app.show_help {
        draw_help(frame);
    }
}

pub fn main_bounds_from_terminal(area: Rect) -> Bounds {
    let chunks = split_main_layout(area);
    let main = chunks[1];
    Bounds {
        x: main.x,
        y: main.y,
        width: main.width,
        height: main.height,
    }
}

fn split_main_layout(area: Rect) -> std::rc::Rc<[Rect]> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);
    chunks
}

fn draw_top_bar(frame: &mut Frame, app: &App, area: Rect) {
    let crumbs = app
        .breadcrumbs()
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" > ");
    let top = Paragraph::new(crumbs).style(Style::default().fg(Color::Yellow));
    frame.render_widget(top, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    // Paint the canvas black so gaps between rects read as intentional margins.
    let bg = Paragraph::new(" ").style(Style::default().bg(Color::Black));
    frame.render_widget(bg, area);

    if !app.is_viewing_other() {
        let Some(state) = app.current_state() else {
            frame.render_widget(Paragraph::new("No data"), area);
            return;
        };

        if state.loading && state.children.is_empty() {
            frame.render_widget(Paragraph::new("Scanning...").block(Block::default().borders(Borders::ALL)), area);
            return;
        }

        if let Some(err) = &state.error {
            frame.render_widget(
                Paragraph::new(err.clone()).block(Block::default().borders(Borders::ALL).title("Error")),
                area,
            );
            return;
        }
    }

    let bounds = Bounds {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height,
    };

    if bounds.width < 4 || bounds.height < 3 {
        return;
    }

    let Some(render_nodes) = app.current_render_nodes() else {
        return;
    };

    let rects = compute_partition_oriented(&app.current_path, &render_nodes, bounds, true);
    for (idx, rect) in rects.iter().enumerate() {
        let is_selected = idx == app.selected_idx;
        let child_loading = app
            .dirs
            .get(&rect.path)
            .map(|s| s.loading)
            .unwrap_or(false);
        draw_rect(frame, rect, is_selected, child_loading);
    }
}

fn draw_rect(frame: &mut Frame, rect: &RectNode, selected: bool, child_loading: bool) {
    let mut area = Rect::new(rect.x, rect.y, rect.width, rect.height);
    // Add a 1-cell gutter on right/bottom edges for visual separation.
    if area.width > 1 {
        area.width = area.width.saturating_sub(1);
    }
    if area.height > 1 {
        area.height = area.height.saturating_sub(1);
    }

    if area.width == 0 || area.height == 0 {
        return;
    }

    if area.width < 4 || area.height < 3 {
        let mini = Paragraph::new(" ").style(if selected {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().bg(if rect.is_dir {
                dir_color_for_path(&rect.path)
            } else {
                Color::DarkGray
            })
        });
        frame.render_widget(mini, area);
        return;
    }

    let kind_prefix = if rect.is_dir { "D" } else { "F" };
    let loading_suffix = if child_loading { " .." } else { "" };
    let title = format!(
        "{}:{} ({}){}",
        kind_prefix,
        truncate(&rect.label, 18),
        human_size(rect.size),
        loading_suffix
    );

    let fill_color = if rect.is_dir {
        dir_color_for_path(&rect.path)
    } else {
        Color::Rgb(200, 200, 200)
    };
    let border_color = if rect.is_dir {
        brighten_neon(fill_color)
    } else {
        Color::Rgb(230, 230, 230)
    };

    // Fill directories completely; keep files as outline-only.
    if rect.is_dir {
        let fill = Paragraph::new(" ").style(Style::default().bg(fill_color));
        frame.render_widget(fill, area);
    }

    let base_style = if rect.is_dir {
        Style::default()
            .fg(text_color_for_bg(fill_color))
            .bg(fill_color)
    } else {
        Style::default().fg(Color::Rgb(210, 210, 210))
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if selected {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            base_style
        })
        .title(title);

    frame.render_widget(block, area);

    if rect.is_other && !rect.other_items.is_empty() && area.width >= 12 && area.height >= 6 {
        let max_lines = usize::from(area.height.saturating_sub(2)).saturating_sub(1);
        let take_n = max_lines.min(rect.other_items.len());
        let mut lines = rect
            .other_items
            .iter()
            .take(take_n)
            .map(|name| format!("- {}", truncate(name, usize::from(area.width.saturating_sub(4)))))
            .collect::<Vec<_>>();
        if rect.other_items.len() > take_n {
            lines.push(format!("+{} more", rect.other_items.len() - take_n));
        }

        let list_fg = if rect.is_dir {
            text_color_for_bg(dir_color_for_path(&rect.path))
        } else {
            Color::White
        };
        let list = Paragraph::new(lines.join("\n")).style(Style::default().fg(list_fg));
        let inner = Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        frame.render_widget(list, inner);
    }
}

fn dir_color_for_path(path: &std::path::Path) -> Color {
    // Neon-only palette.
    const PALETTE: [Color; 12] = [
        Color::Rgb(0, 255, 214),   // neon aqua
        Color::Rgb(57, 255, 20),   // electric lime
        Color::Rgb(255, 49, 146),  // hot neon pink
        Color::Rgb(0, 229, 255),   // neon cyan
        Color::Rgb(255, 111, 0),   // vivid orange
        Color::Rgb(166, 77, 255),  // electric violet
        Color::Rgb(0, 99, 255),    // laser blue
        Color::Rgb(255, 214, 10),  // neon yellow
        Color::Rgb(255, 0, 255),   // magenta
        Color::Rgb(0, 255, 127),   // spring green
        Color::Rgb(255, 20, 60),   // neon crimson
        Color::Rgb(64, 224, 255),  // electric sky
    ];

    let mut h: u64 = 1469598103934665603;
    for b in path.to_string_lossy().as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(1099511628211);
    }
    let idx = (h % PALETTE.len() as u64) as usize;
    PALETTE[idx]
}

fn brighten_neon(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => {
            let boost = |v: u8| v.saturating_add(36);
            Color::Rgb(boost(r), boost(g), boost(b))
        }
        other => other,
    }
}

fn text_color_for_bg(bg: Color) -> Color {
    match bg {
        Color::Rgb(r, g, b) => {
            let luminance = 0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b);
            if luminance > 160.0 {
                Color::Black
            } else {
                Color::White
            }
        }
        _ => Color::White,
    }
}

fn draw_bottom_bar(frame: &mut Frame, app: &App, area: Rect, selected: Option<&RectNode>) {
    let text = if let Some(selected) = selected {
        let parent_size = app
            .current_state()
            .map(|s| if s.size > 0 { s.size } else { s.loaded_size })
            .unwrap_or(0);
        let pct_parent = pct_of(selected.size, parent_size);
        let pct_root = pct_of(selected.size, app.root_size);
        let pct_disk = pct_of(selected.size, app.disk_total);
        format!(
            "{} | size={} | {:.1}% parent | {:.1}% root | {:.1}% disk | du-based view | [c] copy path | [?] help | {}",
            selected.path.display(),
            human_size(selected.size),
            pct_parent,
            pct_root,
            pct_disk,
            app.status
        )
    } else {
        format!("{} | du-based view | [c] copy path | [?] help", app.status)
    };

    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(frame.area(), 70, 60);
    let help = Paragraph::new(
        "Keys:\n[q] quit\nArrow keys move by geometry\n[j]/[k] move by list\n[Enter] or [l] zoom in\n[h], [u], or [Backspace] go parent\n[c] copy selected path\n[?] toggle help",
    )
    .block(Block::default().title("Help").borders(Borders::ALL))
    .wrap(Wrap { trim: true });

    frame.render_widget(Clear, area);
    frame.render_widget(help, area);
}

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    let keep = max.saturating_sub(1);
    format!("{}~", &input[..keep])
}

fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

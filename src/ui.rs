use crate::app::App;
use crate::format::human_size;
use crate::layout::{compute_partition_oriented, Bounds};
use crate::types::RectNode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

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
    let bg = Paragraph::new(" ").style(Style::default().bg(Color::Black));
    frame.render_widget(bg, area);

    let Some(state) = app.current_state() else {
        frame.render_widget(Paragraph::new("No data"), area);
        return;
    };

    if state.loading && state.children.is_empty() {
        let scanning_text = format!("Scanning {}...\n{}", app.spinner_char(), app.status);
        frame.render_widget(
            Paragraph::new(scanning_text)
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL).title("Scanner")),
            area,
        );
        return;
    }

    if let Some(err) = &state.error {
        frame.render_widget(
            Paragraph::new(err.clone()).block(Block::default().borders(Borders::ALL).title("Error")),
            area,
        );
        return;
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
    let max_x = area.x.saturating_add(area.width);
    let max_y = area.y.saturating_add(area.height);
    for (idx, rect) in rects.iter().enumerate() {
        let is_selected = idx == app.selected_idx;
        let child_loading = app
            .dirs
            .get(&rect.path)
            .map(|s| s.loading)
            .unwrap_or(false);
        draw_rect(frame, rect, is_selected, child_loading, max_x, max_y);
    }
}

fn draw_rect(
    frame: &mut Frame,
    rect: &RectNode,
    selected: bool,
    child_loading: bool,
    max_x: u16,
    max_y: u16,
) {
    let mut area = Rect::new(rect.x, rect.y, rect.width, rect.height);
    let touches_right_edge = area.x.saturating_add(area.width) >= max_x;
    let touches_bottom_edge = area.y.saturating_add(area.height) >= max_y;
    if area.width > 1 && !touches_right_edge {
        area.width = area.width.saturating_sub(1);
    }
    if area.height > 1 && !touches_bottom_edge {
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

    let fill_color = dir_color_for_path(&rect.path);
    let border_color = brighten_neon(fill_color);

    if rect.is_dir {
        let fill = Paragraph::new(" ").style(Style::default().bg(fill_color));
        frame.render_widget(fill, area);
    }

    let base_style = if rect.is_dir {
        Style::default().fg(border_color).bg(fill_color)
    } else {
        Style::default().fg(border_color).bg(Color::Black)
    };

    let title_style = if selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else if rect.is_dir {
        Style::default()
            .fg(text_color_for_bg(fill_color))
            .bg(fill_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(Color::Black).add_modifier(Modifier::BOLD)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if rect.is_dir {
            BorderType::Double
        } else {
            BorderType::Plain
        })
        .border_style(if selected {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            base_style
        })
        .title_style(title_style)
        .title(title);

    frame.render_widget(block, area);

    if !rect.is_dir {
        let inner = Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        draw_diagonal_stripes(frame, inner, border_color);
    }
}

fn dir_color_for_path(path: &std::path::Path) -> Color {
    const PALETTE: [Color; 12] = [
        Color::Rgb(0, 255, 214),
        Color::Rgb(57, 255, 20),
        Color::Rgb(255, 49, 146),
        Color::Rgb(0, 229, 255),
        Color::Rgb(255, 111, 0),
        Color::Rgb(166, 77, 255),
        Color::Rgb(0, 99, 255),
        Color::Rgb(255, 214, 10),
        Color::Rgb(255, 0, 255),
        Color::Rgb(0, 255, 127),
        Color::Rgb(255, 20, 60),
        Color::Rgb(64, 224, 255),
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

fn draw_diagonal_stripes(frame: &mut Frame, area: Rect, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let mut lines = Vec::with_capacity(usize::from(area.height));
    for y in 0..area.height {
        let mut line = String::with_capacity(usize::from(area.width));
        for x in 0..area.width {
            if ((x + y) % 3) == 0 {
                line.push('/');
            } else {
                line.push(' ');
            }
        }
        lines.push(line);
    }

    let stripes = Paragraph::new(lines.join("\n")).style(Style::default().fg(color).bg(Color::Black));
    frame.render_widget(stripes, area);
}

fn draw_bottom_bar(frame: &mut Frame, app: &App, area: Rect, selected: Option<&RectNode>) {
    let text = if let Some(state) = app.current_state() {
        let processed = state.children.len();
        if state.loading {
            format!(
                "{} Scanning {} | Calculating files/dirs... | items processed: {}/? | disk: {} | [c] copy path | [?] help",
                app.spinner_char(),
                app.current_path.display(),
                processed,
                human_size(app.disk_total)
            )
        } else {
            let selected_text = selected
                .map(|rect| format!(" | selected: {} ({})", rect.path.display(), human_size(rect.size)))
                .unwrap_or_default();
            format!(
                "Ready | {} | items: {}{} | disk: {} | [c] copy path | [?] help",
                app.current_path.display(),
                processed,
                selected_text,
                human_size(app.disk_total)
            )
        }
    } else {
        format!(
            "{} Scanning {} | Calculating files/dirs... | items processed: 0/? | disk: {} | [c] copy path | [?] help",
            app.spinner_char(),
            app.current_path.display(),
            human_size(app.disk_total)
        )
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

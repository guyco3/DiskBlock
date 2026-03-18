mod actions;
mod app;
mod cache;
mod format;
mod layout;
mod scanner;
mod types;
mod ui;

use actions::{copy_path_to_clipboard, prompt_sudo_auth};
use app::{App, NavDirection};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use scanner::{can_read_dir, spawn_scanner};
use std::collections::HashSet;
use std::env;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// Returns the drawable bounds for the main treemap pane.
fn current_main_bounds(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<crate::layout::Bounds> {
    let size = terminal.size()?;
    let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
    Ok(ui::main_bounds_from_terminal(area))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = env::args().nth(1).unwrap_or_else(|| "/".to_string());
    let root_path = PathBuf::from(root);

    let scanner = spawn_scanner();
    let mut app = App::new(root_path, scanner.clone());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = run_result {
        eprintln!("error: {err}");
    }

    if let Err(err) = app.persist_cache() {
        eprintln!("cache save error: {err}");
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    let mut prompted_sudo_paths: HashSet<PathBuf> = HashSet::new();

    loop {
        app.tick_spinner();

        while let Ok(event) = app.scanner.rx.try_recv() {
            match event {
                crate::types::ScanEvent::PermissionRequired { path } => {
                    if prompted_sudo_paths.contains(&path) {
                        continue;
                    }

                    prompted_sudo_paths.insert(path.clone());
                    app.status = format!(
                        "Need sudo to scan protected entries in {}",
                        path.display()
                    );

                    suspend_terminal(terminal)?;
                    let auth = prompt_sudo_auth();
                    resume_terminal(terminal)?;
                    match auth {
                        Ok(()) => app.rescan_with_sudo(path),
                        Err(err) => app.status = format!("Sudo skipped/failed: {err}"),
                    }
                }
                other => app.on_scan_event(other),
            }
        }

        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down => {
                        app.move_geometric(NavDirection::Down, current_main_bounds(terminal)?);
                    }
                    KeyCode::Up => {
                        app.move_geometric(NavDirection::Up, current_main_bounds(terminal)?);
                    }
                    KeyCode::Left => {
                        app.move_geometric(NavDirection::Left, current_main_bounds(terminal)?);
                    }
                    KeyCode::Right => {
                        app.move_geometric(NavDirection::Right, current_main_bounds(terminal)?);
                    }
                    KeyCode::Char('j') => {
                        app.move_next(current_main_bounds(terminal)?);
                    }
                    KeyCode::Char('k') => app.move_prev(),
                    KeyCode::Char('h') | KeyCode::Char('u') | KeyCode::Backspace => {
                        app.go_parent()
                    }
                    KeyCode::Enter | KeyCode::Char('l') => {
                        let bounds = current_main_bounds(terminal)?;

                        if let Some(selected) = app.selected_rendered_node(bounds) {
                            if selected.kind == crate::types::NodeKind::Directory {
                                if can_read_dir(&selected.path) {
                                    app.enter_node(selected, false);
                                } else {
                                    suspend_terminal(terminal)?;
                                    let auth = prompt_sudo_auth();
                                    resume_terminal(terminal)?;
                                    match auth {
                                        Ok(()) => app.enter_node(selected, true),
                                        Err(err) => app.status = err,
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('c') => {
                        let bounds = current_main_bounds(terminal)?;

                        if let Some(rect) = app.selected_rendered_rect(bounds) {
                            match copy_path_to_clipboard(&rect.path) {
                                Ok(()) => {
                                    app.status =
                                        format!("Copied path to clipboard: {}", rect.path.display())
                                }
                                Err(err) => {
                                    app.status = format!(
                                        "Clipboard unavailable ({err}). Copy manually: {}",
                                        rect.path.display()
                                    )
                                }
                            }
                        }
                    }
                    KeyCode::Char('?') => app.toggle_help(),
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(())
}

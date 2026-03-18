mod actions;
mod app;
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
use std::env;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

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

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        while let Ok(event) = app.scanner.rx.try_recv() {
            app.on_scan_event(event);
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
                        let size = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        let bounds = ui::main_bounds_from_terminal(area);
                        app.move_geometric(NavDirection::Down, bounds);
                    }
                    KeyCode::Up => {
                        let size = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        let bounds = ui::main_bounds_from_terminal(area);
                        app.move_geometric(NavDirection::Up, bounds);
                    }
                    KeyCode::Left => {
                        let size = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        let bounds = ui::main_bounds_from_terminal(area);
                        app.move_geometric(NavDirection::Left, bounds);
                    }
                    KeyCode::Right => {
                        let size = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        let bounds = ui::main_bounds_from_terminal(area);
                        app.move_geometric(NavDirection::Right, bounds);
                    }
                    KeyCode::Char('j') => app.move_next(),
                    KeyCode::Char('k') => app.move_prev(),
                    KeyCode::Char('h') | KeyCode::Char('u') | KeyCode::Backspace => {
                        app.go_parent()
                    }
                    KeyCode::Enter | KeyCode::Char('l') => {
                        let size = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        let bounds = ui::main_bounds_from_terminal(area);

                        if let Some(selected) = app.selected_rendered_node(bounds) {
                            if selected.kind == crate::types::NodeKind::Directory {
                                let is_virtual_other = selected
                                    .path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|n| n == "<Other>")
                                    .unwrap_or(false);

                                if is_virtual_other {
                                    app.enter_node(selected, false);
                                } else if can_read_dir(&selected.path) {
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
                        let size = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        let bounds = ui::main_bounds_from_terminal(area);

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

use std::env;
use std::error::Error;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind, MouseButton};
use crossterm::terminal;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::{TerminalOptions, Viewport};
use time::{OffsetDateTime, macros::format_description};
use ttd::bootstrap::LaunchMode;
use ttd::cli::{Cli, Command};
use ttd::config::ConfigPaths;
use ttd::store::TaskStore;
use ttd::tui::app::{AppMode, AppState};
use ttd::tui::mouse::{DoubleClickTracker, MouseAction, resolve_mouse_action, resolve_scroll_action};
use ttd::tui::render::{LayoutRects, render_session_frame, render_session_frame_with_layout};
use ttd::tui::session::TuiSession;

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    if let Some(command) = cli.command {
        run_command(command, cli.task_dir)?;
    } else {
        run_tui()?;
    }

    Ok(())
}

fn run_command(command: Command, cli_task_dir: Option<PathBuf>) -> io::Result<()> {
    let root = resolve_task_dir(cli_task_dir)?;
    let store = TaskStore::open(root)?;

    match command {
        Command::Add { line } => {
            let line = line.join(" ");
            store.create_task(&line)?;
        }
        Command::List => {
            for task in store.load_all()?.open_tasks {
                println!("{}", task.task.raw);
            }
        }
        Command::Done { id } => {
            store.mark_done_by_name(&id, &today_date()?)?;
        }
        Command::Search { query } => {
            let query_lower = query.to_lowercase();
            for task in store.load_all()?.open_tasks {
                if task.task.raw.to_lowercase().contains(&query_lower) {
                    println!("{}", task.task.raw);
                }
            }
        }
    }

    Ok(())
}

fn run_tui() -> io::Result<()> {
    let paths = ConfigPaths::discover()?;
    let launch_mode = LaunchMode::from_disk(&paths)?;
    let today = today_date()?;

    if env::var_os("TTD_TUI_RENDER_ONCE").is_some() {
        let session = TuiSession::from_launch_mode(launch_mode, &today)?;
        render_tui_once_to_stdout(&session)
    } else {
        let session = TuiSession::from_launch_mode(launch_mode, &today)?;
        run_live_tui(session, &paths)
    }
}

fn resolve_task_dir(cli_task_dir: Option<PathBuf>) -> io::Result<PathBuf> {
    if let Some(dir) = cli_task_dir {
        return Ok(dir);
    }
    if let Some(dir) = env::var_os("TTD_TASK_DIR").map(PathBuf::from) {
        return Ok(dir);
    }
    if let Ok(paths) = ConfigPaths::discover()
        && let Ok(config) = ttd::config::AppConfig::load(&paths)
    {
        return Ok(config.task_dir);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "task directory not found: use --task-dir, set TTD_TASK_DIR, or run the TUI to configure",
    ))
}

fn today_date() -> io::Result<String> {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    now.date()
        .format(format_description!("[year]-[month]-[day]"))
        .map_err(|error| io::Error::other(error.to_string()))
}

fn render_tui_once_to_stdout(session: &TuiSession) -> io::Result<()> {
    let width = 80u16;
    let backend = TestBackend::new(width, 24);
    let mut terminal =
        Terminal::new(backend).expect("test backend terminal creation should not fail");
    let frame = terminal
        .draw(|frame| render_session_frame(frame, session))
        .expect("test backend draw should not fail");
    let cells: Vec<&str> = frame
        .buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    let mut output = String::new();
    for (i, symbol) in cells.iter().enumerate() {
        output.push_str(symbol);
        if (i + 1) % width as usize == 0 {
            output.push('\n');
        }
    }

    io::stdout().write_all(output.as_bytes())
}

const POLL_INTERVAL: Duration = Duration::from_secs(2);

fn run_live_tui(mut session: TuiSession, paths: &ConfigPaths) -> io::Result<()> {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
        ratatui::restore();
        prev_hook(info);
    }));

    let mut terminal = init_live_terminal()?;
    let mut key_buffer = LiveKeyBuffer::new();
    let layout = LayoutRects::default();
    let mut double_click = DoubleClickTracker::new();
    let result = (|| -> io::Result<()> {
        loop {
            terminal.draw(|frame| render_session_frame_with_layout(frame, &session, &layout))?;

            if event::poll(POLL_INTERVAL)? {
                let event = event::read()?;
                if live_tui_control_for_event(&event) == LiveTuiControl::Exit {
                    return Ok(());
                }

                if let Event::Mouse(mouse) = &event {
                    if session.app().editor.is_none()
                        && session.app().save_conflict.is_none()
                        && !session.app().confirm_delete
                        && session.app().mode == AppMode::Main
                    {
                        if let Some(rects) = layout.get() {
                            match mouse.kind {
                                MouseEventKind::Down(MouseButton::Left) => {
                                    if let Some(action) = resolve_mouse_action(
                                        mouse.column,
                                        mouse.row,
                                        &rects,
                                        session.sidebar_items(),
                                    ) {
                                        match action {
                                            MouseAction::SelectSidebar(index) => {
                                                session.dispatch_mouse_sidebar(index);
                                            }
                                            MouseAction::ClickTaskPane { row } => {
                                                if let Some(task_index) =
                                                        session.task_index_for_visual_row(row, rects.task_pane_inner_width)
                                                {
                                                    if double_click.record(task_index) {
                                                        session.dispatch_mouse_task_edit()?;
                                                    } else {
                                                        session.dispatch_mouse_task_select(task_index);
                                                    }
                                                }
                                            }
                                            MouseAction::Scroll { .. } => {}
                                        }
                                    }
                                }
                                MouseEventKind::ScrollUp => {
                                    if let Some(action) =
                                        resolve_scroll_action(mouse.column, mouse.row, &rects, -3)
                                    {
                                        if let MouseAction::Scroll { in_task_pane, delta } = action {
                                            if in_task_pane {
                                                session.apply_task_scroll(delta, rects.visual_line_count, rects.pane_height);
                                            } else {
                                                session.apply_sidebar_scroll(delta);
                                            }
                                        }
                                    }
                                }
                                MouseEventKind::ScrollDown => {
                                    if let Some(action) =
                                        resolve_scroll_action(mouse.column, mouse.row, &rects, 3)
                                    {
                                        if let MouseAction::Scroll { in_task_pane, delta } = action {
                                            if in_task_pane {
                                                session.apply_task_scroll(delta, rects.visual_line_count, rects.pane_height);
                                            } else {
                                                session.apply_sidebar_scroll(delta);
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    continue; // Mouse events don't go through key_buffer
                }

                let Some(key) =
                    key_buffer.token_for_event(&event, app_accepts_compound_tokens(session.app()))
                else {
                    continue;
                };

                session.dispatch_key_with_paths(&key, paths)?;
            } else {
                session.poll_refresh()?;
            }

            if session.app().should_quit {
                return Ok(());
            }
        }
    })();
    let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn init_live_terminal() -> io::Result<ratatui::DefaultTerminal> {
    let (width, height) = terminal::size()?;
    if width == 0 || height == 0 {
        let terminal = ratatui::try_init_with_options(TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 80, 24)),
        })?;
        crossterm::execute!(io::stdout(), terminal::EnterAlternateScreen, EnableMouseCapture)?;
        return Ok(terminal);
    }

    let terminal = ratatui::try_init()?;
    crossterm::execute!(io::stdout(), EnableMouseCapture)?;
    Ok(terminal)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiveTuiControl {
    Continue,
    Exit,
}

fn live_tui_control_for_event(event: &Event) -> LiveTuiControl {
    match event {
        Event::Key(key)
            if key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            LiveTuiControl::Exit
        }
        _ => LiveTuiControl::Continue,
    }
}

#[derive(Debug, Default)]
struct LiveKeyBuffer {
    pending_g: bool,
}

impl LiveKeyBuffer {
    fn new() -> Self {
        Self::default()
    }

    fn token_for_event(&mut self, event: &Event, allow_compound_tokens: bool) -> Option<String> {
        let token = key_token_from_event(event)?;

        if !allow_compound_tokens {
            self.pending_g = false;
            return Some(token);
        }

        match (self.pending_g, token.as_str()) {
            (true, "g") => {
                self.pending_g = false;
                Some("gg".into())
            }
            (true, _) => {
                self.pending_g = false;
                Some(token)
            }
            (false, "g") => {
                self.pending_g = true;
                None
            }
            (false, _) => Some(token),
        }
    }
}

fn app_accepts_compound_tokens(app: &AppState) -> bool {
    app.mode == AppMode::Main
        && !app.search_active
        && !app.confirm_delete
        && app.editor.is_none()
        && app.save_conflict.is_none()
        && app.picker.is_none()
}

fn key_token_from_event(event: &Event) -> Option<String> {
    let Event::Key(key) = event else {
        return None;
    };

    if key.kind != KeyEventKind::Press {
        return None;
    }

    match key.code {
        KeyCode::Backspace => Some("backspace".into()),
        KeyCode::Enter => Some("enter".into()),
        KeyCode::Tab => Some("tab".into()),
        KeyCode::Esc => Some("esc".into()),
        KeyCode::Left => Some("left".into()),
        KeyCode::Right => Some("right".into()),
        KeyCode::Up => Some("up".into()),
        KeyCode::Down => Some("down".into()),
        KeyCode::Home => Some("home".into()),
        KeyCode::End => Some("end".into()),
        KeyCode::Char(character) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Some(format!("ctrl+{}", character.to_ascii_lowercase()))
            } else {
                Some(character.to_string())
            }
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("ttd-main-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn live_tui_only_exits_on_ctrl_c() {
        assert_eq!(
            live_tui_control_for_event(&Event::Key(KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::NONE,
            ))),
            LiveTuiControl::Continue
        );
        assert_eq!(
            live_tui_control_for_event(&Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            ))),
            LiveTuiControl::Continue
        );
        assert_eq!(
            live_tui_control_for_event(&Event::Key(KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
            ))),
            LiveTuiControl::Exit
        );
    }

    #[test]
    fn submitting_welcome_path_saves_config_and_switches_to_main_mode() {
        let root = temp_path("welcome-submit");
        let paths = ConfigPaths::from_root(root.join("config"));
        let task_dir = root.join("todo.txt.d");
        let mut session = TuiSession::welcome("2026-03-30");

        for key in task_dir.display().to_string().chars() {
            session
                .dispatch_key_with_paths(&key.to_string(), &paths)
                .unwrap();
        }
        session.dispatch_key_with_paths("enter", &paths).unwrap();

        assert_eq!(session.app().mode, ttd::tui::app::AppMode::Main);
        assert!(session.app().welcome_input.is_empty());
        assert_eq!(
            fs::read_to_string(&paths.config_file).unwrap(),
            task_dir.display().to_string()
        );
        assert!(task_dir.join("done.txt.d").is_dir());
    }

    #[test]
    fn live_key_buffer_emits_gg_for_double_g_in_normal_main_mode() {
        let mut buffer = LiveKeyBuffer::new();
        let app = ttd::tui::app::AppState::new(ttd::tui::app::AppMode::Main);
        let event = Event::Key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));

        assert_eq!(
            buffer.token_for_event(&event, app_accepts_compound_tokens(&app)),
            None
        );
        assert_eq!(
            buffer.token_for_event(&event, app_accepts_compound_tokens(&app)),
            Some("gg".into())
        );
    }

    #[test]
    fn live_key_buffer_leaves_g_unbuffered_while_text_input_is_active() {
        let mut buffer = LiveKeyBuffer::new();
        let app = ttd::tui::app::AppState::new(ttd::tui::app::AppMode::Welcome);
        let event = Event::Key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));

        assert_eq!(
            buffer.token_for_event(&event, app_accepts_compound_tokens(&app)),
            Some("g".into())
        );
    }
}

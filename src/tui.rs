use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind, EnableMouseCapture, DisableMouseCapture};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, cursor};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

use crate::model::{self, Environment, State};
use crate::notifications::Notifier;
use crate::watcher::RegistryWatcher;

/// Interval between automatic refreshes.
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// A flattened reference to a single unit across all environments, used for
/// cursor-based selection in the TUI.
struct UnitRef {
    env_index: usize,
    unit_index: usize,
}

/// Application state for the TUI.
struct App {
    envs: Vec<Environment>,
    scroll_offset: usize,
    notifier: Notifier,
    /// Index into the flattened list of all units across all environments.
    selected_unit: usize,
}

impl App {
    fn new() -> Self {
        let envs = model::load_all();
        let mut notifier = Notifier::new();
        notifier.process(&envs);
        App {
            envs,
            scroll_offset: 0,
            notifier,
            selected_unit: 0,
        }
    }

    fn refresh(&mut self) {
        self.envs = model::load_all();
        self.notifier.process(&self.envs);
    }

    /// Total number of units across all environments.
    fn total_units(&self) -> usize {
        self.envs.iter().map(|e| e.units.len()).sum()
    }

    /// Move the unit selection cursor up.
    fn select_prev(&mut self) {
        if self.selected_unit > 0 {
            self.selected_unit -= 1;
        }
    }

    /// Move the unit selection cursor down.
    fn select_next(&mut self) {
        let total = self.total_units();
        if total > 0 && self.selected_unit < total - 1 {
            self.selected_unit += 1;
        }
    }

    /// Clamp selected_unit to valid range.
    fn clamp_selection(&mut self) {
        let total = self.total_units();
        if total == 0 {
            self.selected_unit = 0;
        } else if self.selected_unit >= total {
            self.selected_unit = total - 1;
        }
    }

    /// Resolve the flat selected_unit index to an (env_index, unit_index) pair.
    fn selected_unit_ref(&self) -> Option<UnitRef> {
        let mut remaining = self.selected_unit;
        for (env_i, env) in self.envs.iter().enumerate() {
            if remaining < env.units.len() {
                return Some(UnitRef {
                    env_index: env_i,
                    unit_index: remaining,
                });
            }
            remaining -= env.units.len();
        }
        None
    }

    /// Toggle per-unit mute for the currently selected unit.
    fn toggle_selected_unit_mute(&mut self) {
        if let Some(r) = self.selected_unit_ref() {
            let env = &self.envs[r.env_index];
            let unit = &env.units[r.unit_index];
            self.notifier
                .toggle_unit_mute(&env.id, &unit.name);
        }
    }

    /// Toggle per-unit notifications for the currently selected unit.
    fn toggle_selected_unit_notifications(&mut self) {
        if let Some(r) = self.selected_unit_ref() {
            let env = &self.envs[r.env_index];
            let unit = &env.units[r.unit_index];
            self.notifier
                .toggle_unit_notifications(&env.id, &unit.name);
        }
    }

    /// Open browser for the selected unit's port.
    fn open_selected_unit_browser(&self) {
        if let Some(r) = self.selected_unit_ref() {
            let env = &self.envs[r.env_index];
            let unit = &env.units[r.unit_index];
            if let Some(port) = env.port_for(&unit.name) {
                let _ = std::process::Command::new("open")
                    .arg(format!("http://localhost:{port}"))
                    .spawn();
            }
        }
    }

    /// Send SIGHUP to the environment of the currently selected unit.
    fn terminate_selected_env(&self) {
        if let Some(r) = self.selected_unit_ref() {
            let env = &self.envs[r.env_index];
            if env.alive {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(env.pid as i32),
                    nix::sys::signal::Signal::SIGHUP,
                );
            }
        }
    }
}

/// Color for a unit state indicator.
fn state_color(state: &State) -> Color {
    match state {
        State::Running | State::Ready => Color::Green,
        State::Building | State::Starting => Color::Yellow,
        State::Failed => Color::Red,
        State::Stopped => Color::DarkGray,
        State::None | State::Other(_) => Color::Gray,
    }
}

/// Build the content lines for a single environment card.
///
/// `selected_flat` is the globally selected flat unit index.
/// `flat_offset` is the flat index of the first unit in this environment.
fn env_lines(
    env: &Environment,
    notifier: &Notifier,
    selected_flat: usize,
    flat_offset: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Header line: name + ports + elapsed
    let name = env.display_name().to_string();
    let elapsed = env.elapsed_string();

    let mut header_spans: Vec<Span<'static>> = Vec::new();

    // Alive indicator
    if env.alive {
        header_spans.push(Span::styled("● ", Style::default().fg(Color::Green)));
    } else {
        header_spans.push(Span::styled("○ ", Style::default().fg(Color::DarkGray)));
    }

    // Project name — bold
    header_spans.push(Span::styled(
        name,
        Style::default().add_modifier(Modifier::BOLD),
    ));

    // Elapsed — right side (we append as a dim span after a gap)
    header_spans.push(Span::styled(
        format!("  {elapsed}"),
        Style::default().fg(Color::DarkGray),
    ));

    lines.push(Line::from(header_spans));

    // Directory on a second line, dimmed
    let dir_str = env.dir.display().to_string();
    lines.push(Line::from(vec![Span::styled(
        format!("  {dir_str}"),
        Style::default().fg(Color::DarkGray),
    )]));

    // Compute dynamic column widths like the GUI does
    let max_name_len = env
        .units
        .iter()
        .map(|u| u.name.len())
        .max()
        .unwrap_or(0);

    let has_any_port = env.units.iter().any(|u| env.port_for(&u.name).is_some());
    // Ports are displayed as ":<port>" which is at most 6 chars (e.g. ":65535")
    let port_col_w: usize = if has_any_port { 6 } else { 0 };

    let max_state_len = env
        .units
        .iter()
        .map(|u| u.state.to_string().len())
        .max()
        .unwrap_or(0);

    // Units — each on its own line
    for (i, unit) in env.units.iter().enumerate() {
        let flat_index = flat_offset + i;
        let is_selected = flat_index == selected_flat;
        let is_muted = notifier.is_unit_muted(&env.id, &unit.name);
        let is_notif_off = notifier.is_unit_notifications_off(&env.id, &unit.name);

        let indicator = unit.state.display_indicator();
        let color = state_color(&unit.state);

        let name_color = if is_muted { Color::DarkGray } else { Color::White };

        let mut spans: Vec<Span<'static>> = Vec::new();

        // Selection cursor
        if is_selected {
            spans.push(Span::styled(
                "> ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::raw("  "));
        }

        // Per-unit status icons (mute + notification)
        let mute_icon = if is_muted { "\u{1f507}" } else { "\u{1f50a}" };
        let notif_icon = if is_notif_off { "\u{1f515}" } else { "\u{1f514}" };
        spans.push(Span::styled(
            mute_icon.to_string(),
            Style::default().fg(if is_muted { Color::DarkGray } else { Color::White }),
        ));
        spans.push(Span::styled(
            notif_icon.to_string(),
            Style::default().fg(if is_notif_off { Color::DarkGray } else { Color::White }),
        ));
        spans.push(Span::raw(" "));

        // State indicator dot
        spans.push(Span::styled(
            indicator.to_string(),
            Style::default().fg(color),
        ));
        spans.push(Span::raw(" "));

        // Left-align the name in a dynamically computed field
        spans.push(Span::styled(
            format!("{:<width$}", unit.name, width = max_name_len),
            Style::default().fg(name_color),
        ));
        spans.push(Span::raw("  "));

        // Port column (fixed width if any unit has a port)
        if has_any_port {
            if let Some(port) = env.port_for(&unit.name) {
                spans.push(Span::styled(
                    format!("{:<width$}", format!(":{port}"), width = port_col_w),
                    Style::default().fg(Color::Cyan),
                ));
            } else {
                spans.push(Span::raw(format!("{:<width$}", "", width = port_col_w)));
            }
            spans.push(Span::raw("  "));
        }

        // State label (dynamically sized column)
        let state_str = unit.state.to_string();
        spans.push(Span::styled(
            format!("{:<width$}", state_str, width = max_state_len),
            Style::default().fg(color),
        ));

        // Optional detail
        if let Some(ref detail) = unit.detail {
            spans.push(Span::styled(
                format!("  {detail}"),
                Style::default().fg(Color::DarkGray),
            ));
        }

        lines.push(Line::from(spans));
    }

    lines
}

/// Result of building content lines — includes line-to-unit mapping for mouse clicks.
struct ContentLines {
    lines: Vec<Line<'static>>,
    /// Maps each line index to a flat unit index (None for header/separator lines).
    line_to_unit: Vec<Option<usize>>,
}

/// Build all content lines, inserting separators between environments.
fn build_content_lines(
    envs: &[Environment],
    notifier: &Notifier,
    selected_unit: usize,
    width: u16,
) -> ContentLines {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut line_to_unit: Vec<Option<usize>> = Vec::new();

    if envs.is_empty() {
        lines.push(Line::from(Span::styled(
            "No environments found.",
            Style::default().fg(Color::DarkGray),
        )));
        line_to_unit.push(None);
        return ContentLines { lines, line_to_unit };
    }

    let separator: String = "\u{2500}".repeat(width as usize);

    let mut flat_offset: usize = 0;
    for (i, env) in envs.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(Span::styled(
                separator.clone(),
                Style::default().fg(Color::DarkGray),
            )));
            line_to_unit.push(None);
        }
        let env_lines = env_lines(env, notifier, selected_unit, flat_offset);
        // First 2 lines are header + dir path, rest are unit lines
        let header_lines = 2usize.min(env_lines.len());
        for _ in 0..header_lines {
            line_to_unit.push(None);
        }
        for unit_i in 0..env.units.len() {
            line_to_unit.push(Some(flat_offset + unit_i));
        }
        lines.extend(env_lines);
        flat_offset += env.units.len();
    }

    ContentLines { lines, line_to_unit }
}

/// Entry point for the TUI. Called from main.
pub fn run() {
    // Install a panic hook that restores the terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show, DisableMouseCapture);
        original_hook(panic_info);
    }));

    if let Err(e) = run_inner() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run_inner() -> io::Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide, EnableMouseCapture)?;

    // Set terminal title
    execute!(stdout, terminal::SetTitle("sutra"))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut last_refresh = Instant::now();

    // Start file watcher (best-effort)
    let watcher = RegistryWatcher::new().ok();

    loop {
        // Clamp selection after any refresh
        app.clamp_selection();

        // Line-to-unit mapping, populated during draw for mouse click handling
        let mut line_to_unit_map: Vec<Option<usize>> = Vec::new();

        // Draw
        terminal.draw(|frame| {
            let area = frame.area();

            // Split: content area + 1-line footer
            let chunks = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

            let content_area = chunks[0];
            let footer_area = chunks[1];

            // Build content
            let content_data = build_content_lines(
                &app.envs,
                &app.notifier,
                app.selected_unit,
                content_area.width,
            );

            // Auto-scroll to keep the selected unit visible.
            // Find the line index of the selected unit in the content.
            let visible_height = content_area.height as usize;
            if let Some(selected_line) = content_data
                .line_to_unit
                .iter()
                .position(|u| *u == Some(app.selected_unit))
            {
                // If the selected line is above the visible area, scroll up to it.
                if selected_line < app.scroll_offset {
                    app.scroll_offset = selected_line;
                }
                // If the selected line is below the visible area, scroll down so it's
                // the last visible line.
                if visible_height > 0 && selected_line >= app.scroll_offset + visible_height {
                    app.scroll_offset = selected_line - visible_height + 1;
                }
            }

            // Clamp scroll offset to valid range
            let max_scroll = content_data.lines.len().saturating_sub(visible_height);
            if app.scroll_offset > max_scroll {
                app.scroll_offset = max_scroll;
            }

            // Store mapping for mouse click handling (clone before moving lines)
            line_to_unit_map = content_data.line_to_unit;

            // Clamp to u16::MAX to avoid overflow on the Paragraph scroll API.
            let scroll_y = app.scroll_offset.min(u16::MAX as usize) as u16;
            let content = Paragraph::new(content_data.lines)
                .scroll((scroll_y, 0));
            frame.render_widget(content, content_area);

            // Footer
            let mut footer_spans: Vec<Span<'static>> = Vec::new();

            if app.notifier.global_mute {
                footer_spans.push(Span::styled(
                    "\u{1f507} MUTED    ",
                    Style::default().fg(Color::Yellow),
                ));
            }

            if app.notifier.global_notifications_off {
                footer_spans.push(Span::styled(
                    "NOTIF OFF    ",
                    Style::default().fg(Color::Yellow),
                ));
            }

            let mute_label = if app.notifier.global_mute {
                "m unmute"
            } else {
                "m mute"
            };
            let notif_label = if app.notifier.global_notifications_off {
                "n notif on"
            } else {
                "n notif off"
            };
            footer_spans.push(Span::styled(
                format!(
                    "q quit  r refresh  j/k select  {mute_label}  {notif_label}  M unit-mute  N unit-notif  o open  x stop"
                ),
                Style::default().fg(Color::DarkGray),
            ));

            let footer = Paragraph::new(Line::from(footer_spans))
                .alignment(ratatui::layout::Alignment::Right);
            frame.render_widget(footer, footer_area);
        })?;

        // Poll for events (up to the remaining time until next refresh)
        let timeout = REFRESH_INTERVAL
            .checked_sub(last_refresh.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('r') => {
                            app.refresh();
                            last_refresh = Instant::now();
                        }
                        KeyCode::Char('j') | KeyCode::Down => app.select_next(),
                        KeyCode::Char('k') | KeyCode::Up => app.select_prev(),
                        KeyCode::Char('m') => app.notifier.toggle_global_mute(),
                        KeyCode::Char('n') => app.notifier.toggle_global_notifications(),
                        KeyCode::Char('M') => app.toggle_selected_unit_mute(),
                        KeyCode::Char('N') => app.toggle_selected_unit_notifications(),
                        KeyCode::Char('o') => app.open_selected_unit_browser(),
                        KeyCode::Char('x') => app.terminate_selected_env(),
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(_) => {
                        // Map click row to a unit
                        let clicked_line = app.scroll_offset + mouse.row as usize;
                        if let Some(Some(flat_idx)) = line_to_unit_map.get(clicked_line) {
                            app.selected_unit = *flat_idx;
                        }
                    }
                    MouseEventKind::ScrollUp => app.select_prev(),
                    MouseEventKind::ScrollDown => app.select_next(),
                    _ => {}
                },
                _ => {}
            }
        }

        // Check for filesystem events from the watcher
        let mut got_fs_event = false;
        if let Some(ref w) = watcher {
            while w.rx.try_recv().is_ok() {
                got_fs_event = true;
            }
        }

        // Periodic or watcher-triggered refresh
        if got_fs_event || last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh();
            last_refresh = Instant::now();
        }
    }

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        cursor::Show,
        DisableMouseCapture
    )?;

    Ok(())
}

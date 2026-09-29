use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use error::Result;
use models::Connection;
use services::connection_monitor::ConnectionMonitor;
use services::process_ops::{kill_process, parse_pid, KillSignal};
use services::{detect_best_monitor, AddressResolver};
use std::collections::HashMap;
use std::env;
use std::io;
use std::time::{Duration, Instant};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};
use utils::formatter::Formatter;

// Import shared modules
mod error;
mod error_tests;
mod models;
mod services;
mod utils;

/// Layout cache for TUI performance
#[derive(Debug, Clone)]
struct LayoutCache {
    available_width: u16,
    visible_columns: Vec<usize>,
    #[allow(dead_code)]
    column_constraints: Vec<Constraint>,
    last_calculation: Instant,
    last_connection_count: usize,
}

impl LayoutCache {
    fn new() -> Self {
        Self {
            available_width: 0,
            visible_columns: Vec::new(),
            column_constraints: Vec::new(),
            last_calculation: Instant::now(),
            last_connection_count: 0,
        }
    }

    fn is_valid(&self, width: u16, connection_count: usize) -> bool {
        self.available_width == width
            && self.last_calculation.elapsed() < Duration::from_millis(500)
            && (connection_count == 0 || self.last_connection_count == connection_count)
    }
}

/// Menu opened with the right mouse button or `k`: it asks how to signal the
/// process that owns the selected connection.
#[derive(Debug, Clone)]
struct KillMenu {
    pid: String,
    program: String,
    /// Index into [`KillMenu::ITEMS`].
    selected: usize,
}

impl KillMenu {
    /// Number of selectable entries in the menu.
    const COUNT: usize = 3;

    fn items() -> [String; Self::COUNT] {
        [
            format!(
                "{} ({})",
                KillSignal::Term.description(),
                KillSignal::Term.label()
            ),
            format!(
                "{} ({})",
                KillSignal::Force.description(),
                KillSignal::Force.label()
            ),
            "Cancel".to_string(),
        ]
    }

    fn new(conn: &Connection) -> Self {
        Self {
            pid: conn.pid.clone(),
            program: conn.program.clone(),
            selected: 0,
        }
    }

    fn move_selection(&mut self, down: bool) {
        let len = Self::COUNT;
        self.selected = if down {
            (self.selected + 1) % len
        } else {
            (self.selected + len - 1) % len
        };
    }
}

/// Application state for the TUI
struct App {
    connections: Vec<Connection>,
    monitor: Box<dyn ConnectionMonitor>,
    resolver: AddressResolver,
    previous_io: HashMap<String, models::ProcessIO>,
    table_state: TableState,
    last_update: Instant,
    auto_refresh: bool,
    sort_column: usize,
    sort_ascending: bool,
    horizontal_scroll: usize,
    layout_cache: LayoutCache,
    last_render_time: Instant,
    render_count: usize,
    skip_next_render: bool,
    /// Currently displayed kill menu, if any.
    kill_menu: Option<KillMenu>,
    /// Area of the table as drawn by the last frame, used for mouse hit tests.
    table_area: Option<Rect>,
    /// Transient status line (kill results, errors, hints).
    status_message: Option<(String, Instant)>,
}

impl App {
    fn with_monitor(monitor: Box<dyn ConnectionMonitor>) -> Self {
        let mut app = Self {
            connections: Vec::new(),
            monitor,
            resolver: AddressResolver::new(false),
            previous_io: HashMap::new(),
            table_state: TableState::default(),
            last_update: Instant::now(),
            auto_refresh: true,
            sort_column: 6,        // RX column
            sort_ascending: false, // Descending order
            horizontal_scroll: 0,
            layout_cache: LayoutCache::new(),
            last_render_time: Instant::now(),
            render_count: 0,
            skip_next_render: false,
            kill_menu: None,
            table_area: None,
            status_message: None,
        };
        app.update_connections();
        app
    }

    fn update_connections(&mut self) {
        match self.monitor.get_connections() {
            Ok(connections) => {
                match self
                    .monitor
                    .update_connection_rates(connections, &self.previous_io)
                {
                    Ok((updated_connections, current_io)) => {
                        // Skip render if connection count hasn't changed significantly
                        let significant_change = (updated_connections.len() as isize
                            - self.connections.len() as isize)
                            .abs()
                            > 5;

                        self.connections = updated_connections;
                        self.previous_io = current_io;
                        self.last_update = Instant::now();
                        self.sort_connections();

                        // Skip next render if no significant changes to improve performance
                        self.skip_next_render = !significant_change && self.connections.len() > 50;
                    }
                    Err(e) => {
                        // Log error but continue with existing data
                        eprintln!("Failed to update connection rates: {}", e);
                    }
                }
            }
            Err(e) => {
                // Log error but continue with existing data - handle permission errors gracefully
                eprintln!("Failed to get connections: {}", e);
                // Don't update connections on error, keep existing data
                eprintln!("Failed to get connections: {}", e);
            }
        }
    }

    fn sort_connections(&mut self) {
        self.connections.sort_by(|a, b| {
            let ordering = match self.sort_column {
                0 => a.program.cmp(&b.program),
                1 => a.protocol.cmp(&b.protocol),
                2 => a.local.cmp(&b.local),
                3 => a.remote.cmp(&b.remote),
                4 => a.state.cmp(&b.state),
                5 => a.tx_rate.cmp(&b.tx_rate),
                6 => a.rx_rate.cmp(&b.rx_rate),
                7 => a.command.cmp(&b.command),
                _ => std::cmp::Ordering::Equal,
            };

            if self.sort_ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });
    }

    fn next_row(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.connections.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn previous_row(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.connections.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn toggle_sort(&mut self, column: usize) {
        if self.sort_column == column {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = column;
            self.sort_ascending = true;
        }
        self.sort_connections();
    }

    fn scroll_left(&mut self) {
        // Scroll 5 columns at a time for faster navigation
        if self.horizontal_scroll > 0 {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(5);
        }
    }

    fn scroll_right(&mut self) {
        // Scroll 5 columns at a time for faster navigation, but don't exceed bounds
        self.horizontal_scroll = (self.horizontal_scroll + 5).min(7);
    }

    fn toggle_resolver(&mut self) {
        let current_state = self.resolver.get_resolve_hosts();
        self.resolver.set_resolve_hosts(!current_state);
        // Force refresh to update display with new resolver state
        self.update_connections();
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
    }

    fn selected_connection(&self) -> Option<&Connection> {
        self.connections.get(self.table_state.selected()?)
    }

    /// Open the kill menu for the row selected with the keyboard.
    fn open_kill_menu_for_selected(&mut self) {
        match self.selected_connection() {
            Some(conn) => {
                let conn = conn.clone();
                self.open_kill_menu(&conn);
            }
            None => self.set_status("No row selected - use the arrows or right-click a row"),
        }
    }

    /// Open the kill menu for a specific connection.
    fn open_kill_menu(&mut self, conn: &Connection) {
        if parse_pid(&conn.pid).is_err() {
            self.set_status(format!("Cannot kill {}: unknown PID", conn.program));
            return;
        }
        self.kill_menu = Some(KillMenu::new(conn));
    }

    fn close_kill_menu(&mut self) {
        self.kill_menu = None;
    }

    /// Map a terminal row to the connection rendered on it.
    fn row_at(&self, mouse_row: u16) -> Option<usize> {
        let area = self.table_area?;
        // Area layout: top border, header row, data rows..., bottom border
        let first_data_row = area.y + 2;
        let last_data_row = area.y + area.height.saturating_sub(2);
        if mouse_row < first_data_row || mouse_row > last_data_row {
            return None;
        }

        let visible_index = (mouse_row - first_data_row) as usize;
        let index = self.table_state.offset() + visible_index;
        (index < self.connections.len()).then_some(index)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(index) = self.row_at(mouse.row) {
                    self.table_state.select(Some(index));
                    let conn = self.connections[index].clone();
                    self.open_kill_menu(&conn);
                } else if self.kill_menu.is_some() {
                    self.close_kill_menu();
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // A left click dismisses an open menu...
                if self.kill_menu.is_some() {
                    self.close_kill_menu();
                    return;
                }
                // ...otherwise it selects the row under the cursor
                if let Some(index) = self.row_at(mouse.row) {
                    self.table_state.select(Some(index));
                }
            }
            _ => {}
        }
    }

    fn handle_kill_menu_key(&mut self, key: &crossterm::event::KeyEvent) {
        if self.kill_menu.is_none() {
            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_kill_menu(),
            KeyCode::Down | KeyCode::Tab => {
                if let Some(menu) = &mut self.kill_menu {
                    menu.move_selection(true);
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                if let Some(menu) = &mut self.kill_menu {
                    menu.move_selection(false);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_kill_menu_item(None),
            KeyCode::Char(choice @ ('1' | '2' | '3')) => {
                let index = choice as usize - '1' as usize;
                self.activate_kill_menu_item(Some(index));
            }
            _ => {}
        }
    }

    fn activate_kill_menu_item(&mut self, forced_index: Option<usize>) {
        let Some(menu) = self.kill_menu.take() else {
            return;
        };
        let index = forced_index.unwrap_or(menu.selected);

        match index {
            0 => self.execute_kill(&menu.pid, &menu.program, KillSignal::Term),
            1 => self.execute_kill(&menu.pid, &menu.program, KillSignal::Force),
            _ => {}
        }
    }

    fn execute_kill(&mut self, pid: &str, program: &str, signal: KillSignal) {
        match kill_process(pid, signal) {
            Ok(()) => {
                self.set_status(format!("{} sent to {} ({})", signal.label(), program, pid));
                self.update_connections();
            }
            Err(e) => self.set_status(format!("Could not kill {}: {}", program, e)),
        }
    }
}

// Use the consolidated formatter from utils
fn format_bytes(bytes: u64) -> String {
    Formatter::format_bytes(bytes)
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let header_text = vec![Line::from(vec![
        Span::styled(
            "Network Monitor TUI",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("Connections: {}", app.connections.len()),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" | "),
        Span::styled(
            if app.auto_refresh {
                "Auto-refresh: ON"
            } else {
                "Auto-refresh: OFF"
            },
            Style::default().fg(if app.auto_refresh {
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::raw(" | "),
        Span::styled(
            if app.resolver.get_resolve_hosts() {
                "Resolver: ON"
            } else {
                "Resolver: OFF"
            },
            Style::default().fg(if app.resolver.get_resolve_hosts() {
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("Last: {:.1}s ago", app.last_update.elapsed().as_secs_f64()),
            Style::default().fg(Color::Yellow),
        ),
    ])];

    let header =
        tui::widgets::Paragraph::new(header_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Connections table
    let header_cells = [
        "Process(ID)",
        "Protocol",
        "Source",
        "Destination",
        "Status",
        "TX",
        "RX",
        "Path",
    ]
    .iter()
    .enumerate()
    .map(|(i, &title)| {
        let style = if i == app.sort_column {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let arrow = if i == app.sort_column {
            if app.sort_ascending {
                " ↑"
            } else {
                " ↓"
            }
        } else {
            ""
        };

        Span::styled(format!("{}{}", title, arrow), style)
    });

    let _header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::REVERSED))
        .height(1);

    let _rows = app.connections.iter().enumerate().map(|(i, conn)| {
        let color = match conn.protocol.as_str() {
            "tcp" | "tcp6" => Color::Green,
            "udp" | "udp6" => Color::Yellow,
            _ => Color::White,
        };

        let is_selected = app
            .table_state
            .selected()
            .map(|sel| sel == i)
            .unwrap_or(false);

        let style = if is_selected {
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray)
        } else if conn.is_active() {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        let cells = vec![
            Span::raw(conn.get_process_display()),
            Span::raw(&conn.protocol),
            Span::raw(&conn.local),
            Span::raw(&conn.remote),
            Span::raw(&conn.state),
            Span::raw(format_bytes(conn.tx_rate)),
            Span::raw(format_bytes(conn.rx_rate)),
            Span::raw(&conn.command),
        ];

        Row::new(cells).style(style)
    });

    // Calculate visible columns based on horizontal scroll with caching
    let total_columns: usize = 8;
    let available_width = chunks[1].width.saturating_sub(2) as usize; // Subtract borders
    let column_widths = [15, 10, 18, 22, 12, 10, 12, 40]; // Stable minimum widths - increased Path column width
    let start_col = app.horizontal_scroll.min(total_columns.saturating_sub(1));

    // Check if we can use cached layout
    let (visible_columns, remaining_width) = if app
        .layout_cache
        .is_valid(chunks[1].width, app.connections.len())
    {
        (
            app.layout_cache.visible_columns.clone(),
            available_width.saturating_sub(
                app.layout_cache
                    .visible_columns
                    .iter()
                    .enumerate()
                    .map(|(i, &col_idx)| {
                        if i < column_widths.len() {
                            column_widths[col_idx]
                        } else {
                            10
                        }
                    })
                    .sum::<usize>()
                    + app.layout_cache.visible_columns.len().saturating_sub(1),
            ),
        )
    } else {
        // Recalculate layout
        let mut visible_columns = Vec::new();
        let mut current_width = 0;

        // Determine which columns to show - be more conservative to avoid frequent changes
        for (i, &width) in column_widths
            .iter()
            .enumerate()
            .skip(start_col)
            .take(total_columns - start_col)
        {
            // Add small buffer to prevent flickering when width is borderline
            let required_width = width + 2; // +2 for padding and buffer
            if current_width + required_width <= available_width || visible_columns.is_empty() {
                visible_columns.push(i);
                current_width += required_width;
            } else {
                break;
            }
        }

        // If no columns fit, show at least the first one
        if visible_columns.is_empty() && start_col < total_columns {
            visible_columns.push(start_col);
        }

        let remaining_width = available_width.saturating_sub(current_width);

        // Update cache
        app.layout_cache.available_width = chunks[1].width;
        app.layout_cache.visible_columns = visible_columns.clone();
        app.layout_cache.last_calculation = Instant::now();
        app.layout_cache.last_connection_count = app.connections.len();

        (visible_columns, remaining_width)
    };

    // Create header with visible columns only
    let header_titles = [
        "Process(ID)",
        "Protocol",
        "Source",
        "Destination",
        "Status",
        "TX",
        "RX",
        "Path",
    ];
    let visible_header_cells: Vec<_> = visible_columns
        .iter()
        .map(|&col_idx| {
            let style = if col_idx == app.sort_column {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let arrow = if col_idx == app.sort_column {
                if app.sort_ascending {
                    " ↑"
                } else {
                    " ↓"
                }
            } else {
                ""
            };

            let title = if col_idx < header_titles.len() {
                header_titles[col_idx]
            } else {
                ""
            };

            Span::styled(format!("{}{}", title, arrow), style)
        })
        .collect();

    let visible_header = Row::new(visible_header_cells)
        .style(Style::default().add_modifier(Modifier::REVERSED))
        .height(1);

    // Create rows with visible columns only
    let visible_rows = app.connections.iter().enumerate().map(|(i, conn)| {
        let color = match conn.protocol.as_str() {
            "tcp" | "tcp6" => Color::Green,
            "udp" | "udp6" => Color::Yellow,
            _ => Color::White,
        };

        let is_selected = app
            .table_state
            .selected()
            .map(|sel| sel == i)
            .unwrap_or(false);

        let style = if is_selected {
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray)
        } else if conn.is_active() {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        let all_cells = [
            conn.get_process_display(),
            conn.protocol.clone(),
            conn.local.clone(),
            app.resolver.resolve_address(&conn.remote),
            conn.state.clone(),
            format_bytes(conn.tx_rate),
            format_bytes(conn.rx_rate),
            conn.command.clone(),
        ];

        let visible_cells: Vec<_> = visible_columns
            .iter()
            .enumerate()
            .map(|(i, &col_idx)| {
                let cell_content = if col_idx < all_cells.len() {
                    all_cells[col_idx].clone()
                } else {
                    "".to_string()
                };

                // Don't truncate last column - give it full remaining space
                let is_last_column = i == visible_columns.len().saturating_sub(1);
                let max_width = if is_last_column {
                    // For last column, use remaining width or a large number
                    remaining_width.max(100)
                } else if col_idx < column_widths.len() {
                    column_widths[col_idx]
                } else {
                    10
                };

                let truncated = if !is_last_column && cell_content.len() > max_width {
                    format!("{}...", &cell_content[..max_width.saturating_sub(3)])
                } else {
                    cell_content
                };
                Span::raw(truncated)
            })
            .collect();

        Row::new(visible_cells).style(style)
    });

    // Calculate constraints for visible columns with more stable sizing
    let visible_constraints: Vec<_> = visible_columns
        .iter()
        .enumerate()
        .map(|(i, &col_idx)| {
            // Give the last column the remaining width
            if i == visible_columns.len().saturating_sub(1) && remaining_width > 0 {
                Constraint::Min(remaining_width as u16)
            } else if col_idx < column_widths.len() {
                // Use fixed widths for better stability
                Constraint::Length(column_widths[col_idx] as u16)
            } else {
                Constraint::Length(10)
            }
        })
        .collect();

    let table = if !visible_constraints.is_empty() {
        Table::new(visible_rows, visible_constraints)
            .header(visible_header)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Network Connections (scroll: ← → | col {}/{})",
                start_col + 1,
                total_columns
            )))
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    } else {
        // Fallback table if no columns fit
        let empty_rows: Vec<Row> = vec![];
        Table::new(empty_rows, [Constraint::Min(10)])
            .header(Row::new([Span::raw("No space")]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Network Connections"),
            )
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    };

    f.render_stateful_widget(table, chunks[1], &mut app.table_state);

    // Remember where the table was drawn so mouse clicks can be mapped back
    // to rows on the next event.
    app.table_area = Some(chunks[1]);

    // Kill confirmation menu (right-click / k)
    if let Some(menu) = &app.kill_menu {
        let area = kill_menu_area(app.table_area.unwrap_or(chunks[1]), menu);
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(format!("Kill {} (PID {})", menu.program, menu.pid));

        let items: Vec<Line> = KillMenu::items()
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == menu.selected;
                let label = if is_selected {
                    format!("\u{276f} {}", item)
                } else {
                    format!("  {}", item)
                };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Line::from(Span::styled(label, style))
            })
            .collect();

        f.render_widget(Paragraph::new(items).block(block), area);
    }

    // Footer with help + transient status
    let mut footer_spans = vec![
        Span::styled("Keys: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("q", Style::default().fg(Color::Red)),
        Span::raw(":quit "),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw(":resolver "),
        Span::styled("R", Style::default().fg(Color::Cyan)),
        Span::raw(":refresh "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw(":auto-refresh "),
        Span::styled("\u{2191}\u{2193}", Style::default().fg(Color::Green)),
        Span::raw(":navigate "),
        Span::styled("k/RMB", Style::default().fg(Color::Red)),
        Span::raw(":kill "),
        Span::styled("\u{2190}\u{2192}", Style::default().fg(Color::Blue)),
        Span::raw(":scroll(5) "),
        Span::styled("1-8", Style::default().fg(Color::Magenta)),
        Span::raw(":sort "),
    ];

    if let Some((message, at)) = &app.status_message {
        if at.elapsed() < Duration::from_secs(5) {
            footer_spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
            footer_spans.push(Span::styled(
                message.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    let footer = tui::widgets::Paragraph::new(Line::from(footer_spans))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Centered area large enough for the kill menu.
fn kill_menu_area(area: Rect, menu: &KillMenu) -> Rect {
    let title_width = menu.program.chars().count() + menu.pid.chars().count() + 14;
    let item_width = KillMenu::items()
        .iter()
        .map(|item| item.chars().count() + 4)
        .max()
        .unwrap_or(20);
    // Never overflow u16 and never exceed the available space
    let wanted = title_width.max(item_width).clamp(24, 240) as u16 + 2;
    let width = wanted.min(area.width.saturating_sub(2));
    let height = ((KillMenu::COUNT as u16) + 2).min(area.height.saturating_sub(2));

    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    // Check for --version argument
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--version" {
        println!("nmt version {}", VERSION);
        return Ok(());
    }

    // Create monitor first (may fail if eBPF unavailable, e.g. no permissions)
    // Must happen BEFORE terminal setup so error messages display cleanly.
    let monitor = detect_best_monitor().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        eprintln!("Run with sudo or set capabilities: sudo setcap cap_bpf,cap_net_admin,cap_perfmon+ep <binary>");
        std::process::exit(1);
    });

    // Try to enable raw mode with better error handling
    match enable_raw_mode() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: Cannot initialize terminal.");
            eprintln!("This usually means you're not in a real terminal.");
            eprintln!("Try running 'nmt' in a proper terminal, not an IDE or script.");
            eprintln!("Technical details: {}", e);
            std::process::exit(1);
        }
    }
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::with_monitor(monitor);
    let mut last_tick = Instant::now();

    let mut last_input_time = Instant::now();
    let mut needs_data_update = false;

    loop {
        // Check for user input first - this is the priority
        let timeout = Duration::from_millis(16); // ~60 FPS

        if crossterm::event::poll(timeout)? {
            last_input_time = Instant::now();

            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if app.kill_menu.is_some() {
                        // The confirmation menu consumes every key while open
                        app.handle_kill_menu_key(&key);
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('k') => app.open_kill_menu_for_selected(),
                        KeyCode::Char('r') => app.toggle_resolver(),
                        KeyCode::Char('R') => needs_data_update = true, // Mark for update, don't block
                        KeyCode::Char('a') => app.auto_refresh = !app.auto_refresh,
                        KeyCode::Up => app.previous_row(),
                        KeyCode::Down => app.next_row(),
                        KeyCode::Left => {
                            if key.modifiers.contains(KeyModifiers::SHIFT)
                                || key.modifiers.contains(KeyModifiers::CONTROL)
                            {
                                app.horizontal_scroll = app.horizontal_scroll.saturating_sub(7);
                            // Fast scroll to start
                            } else {
                                app.scroll_left(); // Normal scroll moves 5 columns
                            }
                        }
                        KeyCode::Right => {
                            if key.modifiers.contains(KeyModifiers::SHIFT)
                                || key.modifiers.contains(KeyModifiers::CONTROL)
                            {
                                app.horizontal_scroll = 7; // Fast scroll to end
                            } else {
                                app.scroll_right(); // Normal scroll moves 5 columns
                            }
                        }
                        KeyCode::Char('1') => app.toggle_sort(0),
                        KeyCode::Char('2') => app.toggle_sort(1),
                        KeyCode::Char('3') => app.toggle_sort(2),
                        KeyCode::Char('4') => app.toggle_sort(3),
                        KeyCode::Char('5') => app.toggle_sort(4),
                        KeyCode::Char('6') => app.toggle_sort(5),
                        KeyCode::Char('7') => app.toggle_sort(6),
                        KeyCode::Char('8') => app.toggle_sort(7),
                        KeyCode::Home => app.horizontal_scroll = 0,
                        KeyCode::End => app.horizontal_scroll = 7, // Last column index
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }

        // Only update data when user is idle AND we need to update
        if needs_data_update
            || (app.auto_refresh
                && last_input_time.elapsed() >= Duration::from_millis(500)
                && last_tick.elapsed() >= Duration::from_secs(2))
        {
            app.update_connections();
            last_tick = Instant::now();
            needs_data_update = false;
        }

        // Skip rendering if no significant changes to improve performance
        if app.skip_next_render && app.connections.len() > 50 {
            app.skip_next_render = false;
        } else {
            // Always draw last - this ensures instant UI response
            terminal.draw(|f| ui(f, &mut app))?;

            // Track render performance
            app.render_count += 1;
            let now = Instant::now();
            if now.duration_since(app.last_render_time).as_secs() >= 5 {
                let fps = app.render_count as f64
                    / now.duration_since(app.last_render_time).as_secs_f64();
                if fps < 30.0 {
                    eprintln!("Performance warning: Low FPS ({:.1}) detected", fps);
                }
                app.render_count = 0;
                app.last_render_time = now;
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

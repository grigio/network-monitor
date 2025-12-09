use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use error::Result;
use models::Connection;
use services::{AddressResolver, NetworkService};
use std::collections::HashMap;
use std::env;
use std::io;
use std::time::{Duration, Instant};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
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
#[allow(dead_code)]
struct LayoutCache {
    available_width: u16,
    visible_columns: Vec<usize>,
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

/// Application state for the TUI
struct App {
    connections: Vec<Connection>,
    network_service: NetworkService,
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
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            connections: Vec::new(),
            network_service: NetworkService::new(),
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
        };
        app.update_connections();
        app
    }

    fn update_connections(&mut self) {
        match self.network_service.get_connections() {
            Ok(connections) => {
                match self
                    .network_service
                    .update_connection_rates(connections, &self.previous_io)
                {
                    Ok((updated_connections, current_io)) => {
                        // Skip render if connection count hasn't changed significantly
                        let significant_change = (updated_connections.len() as isize - self.connections.len() as isize).abs() > 5;
                        
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
    let (visible_columns, remaining_width) = if app.layout_cache.is_valid(chunks[1].width, app.connections.len()) {
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

    // Footer with help
    let footer_text = vec![Line::from(vec![
        Span::styled("Keys: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("q", Style::default().fg(Color::Red)),
        Span::raw(":quit "),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw(":resolver "),
        Span::styled("R", Style::default().fg(Color::Cyan)),
        Span::raw(":refresh "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw(":auto-refresh "),
        Span::styled("↑↓", Style::default().fg(Color::Green)),
        Span::raw(":navigate "),
        Span::styled("←→", Style::default().fg(Color::Blue)),
        Span::raw(":scroll(5) "),
        Span::styled("Shift+←→", Style::default().fg(Color::Blue)),
        Span::raw(":jump "),
        Span::styled("Home/End", Style::default().fg(Color::Blue)),
        Span::raw(":jump "),
        Span::styled("1-8", Style::default().fg(Color::Magenta)),
        Span::raw(":sort "),
    ])];

    let footer =
        tui::widgets::Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    // Check for --version argument
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--version" {
        println!("nmt version {}", VERSION);
        return Ok(());
    }

    // Try to enable raw mode with better error handling
    match enable_raw_mode() {
        Ok(()) => {
            // Continue with terminal setup
        }
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

    let mut app = App::new();
    let mut last_tick = Instant::now();

    let mut last_input_time = Instant::now();
    let mut needs_data_update = false;

    loop {
        // Check for user input first - this is the priority
        let timeout = Duration::from_millis(16); // ~60 FPS

        if crossterm::event::poll(timeout)? {
            last_input_time = Instant::now();

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
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
                let fps = app.render_count as f64 / now.duration_since(app.last_render_time).as_secs_f64();
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

use gtk4 as gtk;
use adw::{prelude::*, Application, ApplicationWindow, HeaderBar, StatusPage, AboutWindow};
use gtk::{Grid, Button, Label, ScrolledWindow, Orientation, Align, MenuButton, gio::Menu};
use glib::timeout_add_seconds_local;
use std::collections::HashMap;
use std::process::Command;
use std::fs;
use std::thread;
use std::sync::{Arc, Mutex};
use regex::Regex;
use std::os::unix::process::ExitStatusExt;

#[derive(Debug, Clone)]
struct Connection {
    protocol: String,
    state: String,
    local: String,
    remote: String,
    program: String,
    pid: String,
    rx_rate: u64,
    tx_rate: u64,
}

#[derive(Debug, Clone)]
struct ProcessIO {
    rx: u64,
    tx: u64,
}

struct NetworkMonitorWindow {
    window: ApplicationWindow,
    grid: Grid,
    status_bar: StatusPage,
    resolve_toggle: gtk::CheckButton,
    header_buttons: Arc<Mutex<Vec<Button>>>,
    prev_io: Arc<Mutex<HashMap<String, ProcessIO>>>,
    resolution_cache: Arc<Mutex<HashMap<String, String>>>,
    resolution_pending: Arc<Mutex<std::collections::HashSet<String>>>,
    sort_column: Arc<Mutex<usize>>,
    sort_ascending: Arc<Mutex<bool>>,
    resolve_hosts: Arc<Mutex<bool>>,
    row_widgets: Arc<Mutex<Vec<Label>>>,
    selected_row: Arc<Mutex<Option<usize>>>,
}

impl NetworkMonitorWindow {
    fn new(app: &Application) -> Arc<Self> {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Network Monitor")
            .default_width(600)
            .default_height(800)
            .build();

        // Set up Adwaita style manager
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::Default);

        // Create Grid for better control
        let grid = Grid::builder()
            .column_spacing(8)
            .row_spacing(2)
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .margin_bottom(8)
            .build();
        
        let status_bar = StatusPage::builder()
            .title("Network Monitor")
            .description("Monitoring network connections...")
            .build();

        let resolve_toggle = gtk::CheckButton::builder()
            .label("Resolve Hostnames")
            .active(true)
            .build();

        let monitor = Arc::new(NetworkMonitorWindow {
            window,
            grid,
            status_bar,
            resolve_toggle,
            header_buttons: Arc::new(Mutex::new(Vec::new())),
            prev_io: Arc::new(Mutex::new(HashMap::new())),
            resolution_cache: Arc::new(Mutex::new(HashMap::new())),
            resolution_pending: Arc::new(Mutex::new(std::collections::HashSet::new())),
            sort_column: Arc::new(Mutex::new(6)),
            sort_ascending: Arc::new(Mutex::new(false)),
            resolve_hosts: Arc::new(Mutex::new(true)),
            row_widgets: Arc::new(Mutex::new(Vec::new())),
            selected_row: Arc::new(Mutex::new(None)),
        });

        monitor.setup_grid();
        monitor.setup_ui();
        monitor.start_monitoring();
        monitor
    }

    fn setup_grid(self: &Arc<Self>) {
        // Create header row with clickable columns for sorting
        let headers = [
            ("Process(ID)", 0),
            ("Protocol", 1),
            ("Source", 2),
            ("Destination", 3),
            ("Status", 4),
            ("TX", 5),
            ("RX", 6),
            ("Path", 7),
        ];

        for (text, col) in headers {
            let button = Button::builder()
                .label(text)
                .build();
            button.add_css_class("heading");
            button.add_css_class("flat");
            
            // Connect click handler for sorting
            let monitor_clone = self.clone();
            let col_index = col;
            button.connect_clicked(move |_| {
                let mut sort_col = monitor_clone.sort_column.lock().unwrap();
                let mut sort_asc = monitor_clone.sort_ascending.lock().unwrap();
                
                if *sort_col == col_index {
                    *sort_asc = !*sort_asc;
                } else {
                    *sort_col = col_index;
                    *sort_asc = true;
                }
                
                drop(sort_col);
                drop(sort_asc);
                
                let monitor_clone2 = monitor_clone.clone();
                glib::idle_add_local_once(move || {
                    monitor_clone2.update_connections();
                });
            });
            
            self.grid.attach(&button, col as i32, 0, 1, 1);
            
            // Store header buttons for styling
            self.header_buttons.lock().unwrap().push(button);
        }
    }

    fn setup_ui(self: &Arc<Self>) {
        // Apply custom CSS
        self.apply_custom_css();

        // Create main box with better spacing
        let main_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .build();

        self.window.set_content(Some(&main_box));

        // Enhanced header bar with better styling
        let title_label = Label::builder()
            .label("Network Monitor")
            .build();
        title_label.add_css_class("title");
        
        let header_bar = HeaderBar::builder()
            .title_widget(&title_label)
            .build();
        header_bar.add_css_class("flat");

        // Create enhanced menu button
        let menu_button = MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Menu")
            .build();
        menu_button.add_css_class("flat");
        menu_button.add_css_class("image-button");
        menu_button.set_margin_end(6);
        let menu_model = self.create_menu_model();
        menu_button.set_menu_model(Some(&menu_model));
        header_bar.pack_end(&menu_button);

        main_box.append(&header_bar);

        // Main content area with improved layout
        let content_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(16)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        main_box.append(&content_box);

        // Create enhanced scrolled window with Grid
        let scrolled = ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .min_content_height(300)
            .max_content_height(600)
            .build();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled.add_css_class("card");
        scrolled.add_css_class("view");
        content_box.append(&scrolled);
        scrolled.set_child(Some(&self.grid));

        // Enhanced status bar with reduced spacing
        self.status_bar.add_css_class("compact");
        self.status_bar.set_margin_top(4);
        self.status_bar.set_margin_bottom(2);
        content_box.append(&self.status_bar);

        // Bottom control panel with reduced spacing
        let control_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(8)
            .margin_end(8)
            .halign(Align::Center)
            .build();
        control_box.add_css_class("toolbar");
        control_box.add_css_class("inline-toolbar");
        content_box.append(&control_box);

        // Enhanced host resolution checkbox with better styling
        self.resolve_toggle.set_tooltip_text(Some("Toggle hostname resolution"));
        self.resolve_toggle.add_css_class("flat");
        
        let resolution_cache = self.resolution_cache.clone();
        let resolve_hosts_field = self.resolve_hosts.clone();
        self.resolve_toggle.connect_toggled(move |button| {
            let resolve_hosts = button.is_active();
            *resolve_hosts_field.lock().unwrap() = resolve_hosts;
            if !resolve_hosts {
                resolution_cache.lock().unwrap().clear();
            }
        });
        control_box.append(&self.resolve_toggle);

        // Update status
        self.update_status(0, 0);
    }

    fn apply_custom_css(&self) {
        let css_provider = gtk::CssProvider::new();
        let css = r#"
            .title {
                font-size: 1.2em;
                font-weight: 700;
                color: @headerbar_fg_color;
                margin: 0 8px;
                transition: all 150ms ease;
            }
            

            
            .card {
                border: 1px solid alpha(@borders, 0.2);
                border-radius: 8px;
                background: @view_bg_color;
                box-shadow: 0 1px 3px alpha(@shade_color, 0.1);
            }
            
            .view {
                background: @view_bg_color;
            }
            
            .toolbar {
                background: alpha(@headerbar_bg_color, 0.3);
                border-radius: 6px;
                padding: 6px 10px;
                border: 1px solid alpha(@borders, 0.15);
            }
            
            .inline-toolbar {
                background: alpha(@theme_bg_color, 0.5);
                border: 1px solid alpha(@borders, 0.1);
            }
            
            .heading {
                font-weight: 600;
                color: @headerbar_fg_color;
                font-size: 0.85em;
                text-transform: uppercase;
                letter-spacing: 0.3px;
                opacity: 0.8;
            }
            
            .badge {
                background: alpha(@accent_bg_color, 0.15);
                border-radius: 3px;
                padding: 1px 4px;
                font-weight: 600;
                font-size: 0.8em;
                border: 1px solid alpha(@accent_bg_color, 0.2);
            }
            
            .success {
                color: @success_color;
                background: alpha(@success_bg_color, 0.08);
                border-color: alpha(@success_bg_color, 0.15);
            }
            
            .warning {
                color: @warning_color;
                background: alpha(@warning_bg_color, 0.08);
                border-color: alpha(@warning_bg_color, 0.15);
            }
            
            .error {
                color: @error_color;
                font-weight: 600;
                background: alpha(@error_bg_color, 0.05);
                border-color: alpha(@error_bg_color, 0.1);
            }
            
            .accent {
                color: @accent_color;
                font-weight: 600;
                background: alpha(@accent_bg_color, 0.05);
                border-color: alpha(@accent_bg_color, 0.1);
            }
            
            .caption {
                font-size: 0.85em;
                opacity: 0.85;
            }
            
            .caption-heading {
                font-weight: 700;
                font-size: 0.9em;
            }
            
            .dim-label {
                opacity: 0.55;
                font-style: italic;
            }
            
            grid {
                background: @view_bg_color;
                border-radius: 6px;
                padding: 6px;
            }
            
            label {
                padding: 3px 5px;
                margin: 1px;
                border-radius: 2px;
                border: none;
                background: rgba(255, 255, 255, 0.05);
            }
            
            label:hover {
                background: rgba(255, 255, 255, 0.08);
            }
            
            button {
                margin: 1px;
                border-radius: 4px;
                transition: all 150ms ease;
                min-width: 32px;
                min-height: 32px;
            }
            
            button:hover {
                background: alpha(@accent_bg_color, 0.08);
                border-color: alpha(@accent_bg_color, 0.15);
            }
            
            button:active {
                background: alpha(@accent_bg_color, 0.12);
                border-color: alpha(@accent_bg_color, 0.2);
            }
            
            .flat {
                background: transparent;
                border-color: transparent;
            }
            
            .flat:hover {
                background: alpha(@accent_bg_color, 0.1);
                border-color: alpha(@accent_bg_color, 0.15);
            }
            
            .image-button {
                padding: 4px;
            }
            
            .toggle {
                padding: 6px 12px;
                font-weight: 500;
            }
            
            .toggle:checked {
                background: @accent_bg_color;
                color: @accent_fg_color;
                border-color: @accent_bg_color;
            }
            
            statuspage {
                padding: 24px;
            }
            
            statuspage > title {
                font-size: 1.1em;
                font-weight: 600;
            }
            
            statuspage > description {
                font-size: 0.9em;
                opacity: 0.7;
            }
            
            .selected {
                background: alpha(@accent_bg_color, 0.15);
                border: 1px solid alpha(@accent_bg_color, 0.3);
                color: @accent_color;
                font-weight: 600;
            }
        "#;
        
        css_provider.load_from_string(css);
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    fn create_menu_model(&self) -> Menu {
        let menu = Menu::new();
        menu.append(Some("Refresh"), Some("app.refresh"));
        menu.append(Some("About"), Some("app.about"));
        menu
    }

    fn get_connections(&self) -> Vec<Connection> {
        let output = Command::new("ss")
            .args(&["-tulnape"])
            .output()
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::from_raw(1),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().skip(1).collect();

        let mut connections = Vec::new();
        let pid_regex = Regex::new(r"pid=(\d+)").unwrap();
        let prog_regex = Regex::new(r#""([^"]+)""#).unwrap();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                continue;
            }

            let protocol = parts[0].to_string();
            let state = parts[1].to_string();
            let local_addr = parts[4].to_string();
            let remote_addr = parts[5].to_string();

            let mut program = "N/A".to_string();
            let mut pid = "N/A".to_string();

            for part in &parts {
                if part.starts_with("users:((") {
                    if let Some(caps) = pid_regex.captures(part) {
                        pid = caps[1].to_string();
                    }
                    if let Some(caps) = prog_regex.captures(part) {
                        program = caps[1].to_string();
                    }
                    break;
                }
            }

            connections.push(Connection {
                protocol,
                state,
                local: local_addr,
                remote: remote_addr,
                program,
                pid,
                rx_rate: 0,
                tx_rate: 0,
            });
        }

        connections
    }

    fn get_process_io(&self, pid: &str) -> ProcessIO {
        let io_path = format!("/proc/{}/io", pid);
        if let Ok(io_data) = fs::read_to_string(&io_path) {
            let mut rx_bytes = 0u64;
            let mut tx_bytes = 0u64;

            for line in io_data.lines() {
                if line.starts_with("rchar:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        rx_bytes = value.parse().unwrap_or(0);
                    }
                } else if line.starts_with("wchar:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        tx_bytes = value.parse().unwrap_or(0);
                    }
                }
            }

            ProcessIO { rx: rx_bytes, tx: tx_bytes }
        } else {
            ProcessIO { rx: 0, tx: 0 }
        }
    }

    fn get_process_path(&self, pid: &str) -> String {
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        if let Ok(cmdline) = fs::read_to_string(&cmdline_path) {
            if !cmdline.is_empty() {
                cmdline.replace('\0', " ")
            } else {
                format!("[{}]", pid)
            }
        } else {
            "N/A".to_string()
        }
    }

    fn resolve_address(&self, addr: &str) -> String {
        if addr == "0.0.0.0:*" || addr == "*:*" || addr == "[::]:*" {
            return "ANY".to_string();
        } else if addr.starts_with("127.0.0.1:") || addr.starts_with("[::1]:") {
            return "LOCALHOST".to_string();
        } else if addr.starts_with("224.0.0.251:") {
            return "MDNS".to_string();
        }

        let resolve_hosts = *self.resolve_hosts.lock().unwrap();
        if !resolve_hosts {
            return addr.to_string();
        }

        // Check cache first
        {
            let cache = self.resolution_cache.lock().unwrap();
            if let Some(resolved) = cache.get(addr) {
                return resolved.clone();
            }
        }

        // Extract IP address and port
        let (ip_part, port) = if let Some(last_colon) = addr.rfind(':') {
            let ip_with_brackets = &addr[..last_colon];
            let port = &addr[last_colon+1..];
            
            let ip_part = if ip_with_brackets.starts_with('[') && ip_with_brackets.ends_with(']') {
                &ip_with_brackets[1..ip_with_brackets.len()-1]
            } else {
                ip_with_brackets
            };
            
            (ip_part.to_string(), port.to_string())
        } else {
            (addr.to_string(), "".to_string())
        };

        // Start async resolution if not already pending
        {
            let mut pending = self.resolution_pending.lock().unwrap();
            if !pending.contains(&ip_part) {
                pending.insert(ip_part.clone());
                
                let addr = addr.to_string();
                let resolution_cache = self.resolution_cache.clone();
                let resolution_pending = self.resolution_pending.clone();
                
                thread::spawn(move || {
                    // Simple hostname resolution using host command
                    let resolved = match std::process::Command::new("host")
                        .arg(&ip_part)
                        .output()
                    {
                        Ok(output) => {
                            let output_str = String::from_utf8_lossy(&output.stdout);
                            // Simple parsing for hostname
                            let mut result = addr.clone();
                            for line in output_str.lines() {
                                if line.contains("domain name pointer") || line.contains("is an alias for") {
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    for (i, part) in parts.iter().enumerate() {
                                        if *part == "pointer" || *part == "alias" {
                                            if i + 1 < parts.len() {
                                                let hostname = parts[i + 1].trim_end_matches('.');
                                                if port.is_empty() {
                                                    result = hostname.to_string();
                                                } else {
                                                    result = format!("{}:{}", hostname, port);
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            result
                        }
                        Err(_) => addr.clone(),
                    };

                    // Update cache
                    {
                        let mut cache = resolution_cache.lock().unwrap();
                        cache.insert(addr.clone(), resolved);
                    }
                    
                    // Remove from pending
                    {
                        let mut pending = resolution_pending.lock().unwrap();
                        pending.remove(&ip_part);
                    }
                });
            }
        }

        addr.to_string()
    }

    fn format_bytes(&self, bytes_val: u64) -> String {
        let mut bytes_val = bytes_val as f64;
        let units = ["B", "KB", "MB", "GB"];
        
        for unit in &units {
            if bytes_val < 1024.0 {
                return format!("{:.1}{}/s", bytes_val, unit);
            }
            bytes_val /= 1024.0;
        }
        format!("{:.1}TB/s", bytes_val)
    }

    fn update_status(&self, total: usize, active: usize) {
        if total > 0 {
            self.status_bar.set_description(Some(&format!("{} total connections, {} active", total, active)));
        } else {
            self.status_bar.set_description(Some("Monitoring network connections..."));
        }
    }

    fn sort_connections(&self, connections: Vec<Connection>) -> Vec<Connection> {
        if connections.is_empty() {
            return connections;
        }

        let sort_column = *self.sort_column.lock().unwrap();
        let sort_ascending = *self.sort_ascending.lock().unwrap();

        let mut sorted_connections = connections;

        sorted_connections.sort_by(|a, b| {
            let comparison = match sort_column {
                0 => format!("{}({})", a.program, a.pid).cmp(&format!("{}({})", b.program, b.pid)),
                1 => a.protocol.cmp(&b.protocol),
                2 => self.resolve_address(&a.local).cmp(&self.resolve_address(&b.local)),
                3 => self.resolve_address(&a.remote).cmp(&self.resolve_address(&b.remote)),
                4 => a.state.cmp(&b.state),
                5 => a.tx_rate.cmp(&b.tx_rate),
                6 => a.rx_rate.cmp(&b.rx_rate),
                7 => {
                    let path_a = if a.pid != "N/A" { self.get_process_path(&a.pid) } else { "N/A".to_string() };
                    let path_b = if b.pid != "N/A" { self.get_process_path(&b.pid) } else { "N/A".to_string() };
                    path_a.cmp(&path_b)
                },
                _ => std::cmp::Ordering::Equal,
            };

            if sort_ascending {
                comparison
            } else {
                comparison.reverse()
            }
        });

        sorted_connections
    }

    fn update_connections(self: &Arc<Self>) {
        // Clear existing grid content (except headers)
        {
            let mut row_widgets = self.row_widgets.lock().unwrap();
            for widget in row_widgets.iter() {
                self.grid.remove(widget);
            }
            row_widgets.clear();
        }

        // Get connections
        let connections = self.get_connections();

        // Update I/O data for rate calculations
        let mut current_io = HashMap::new();
        let mut updated_connections = Vec::new();

        for mut conn in connections {
            if conn.pid != "N/A" {
                let io = self.get_process_io(&conn.pid);
                let pid_key = conn.pid.clone();
                
                // Calculate rates based on previous I/O data
                {
                    let prev_io = self.prev_io.lock().unwrap();
                    if let Some(prev) = prev_io.get(&pid_key) {
                        conn.rx_rate = io.rx.saturating_sub(prev.rx);
                        conn.tx_rate = io.tx.saturating_sub(prev.tx);
                    } else {
                        conn.rx_rate = 0;
                        conn.tx_rate = 0;
                    }
                }
                
                current_io.insert(pid_key, io);
            } else {
                conn.rx_rate = 0;
                conn.tx_rate = 0;
            }
            
            updated_connections.push(conn);
        }

        // Update previous I/O data for next iteration
        {
            let mut prev_io = self.prev_io.lock().unwrap();
            *prev_io = current_io;
        }

        // Filter out localhost connections
        let filtered_connections: Vec<Connection> = updated_connections
            .into_iter()
            .filter(|conn| self.resolve_address(&conn.remote) != "LOCALHOST")
            .collect();

        // Sort connections
        let sorted_connections = self.sort_connections(filtered_connections);

        let mut active_connections = 0;
        let mut row = 1; // Start from row 1 (row 0 is headers)

        for conn in &sorted_connections {
            // Format display values
            let prog_pid = if conn.pid != "N/A" {
                format!("{}({})", conn.program, conn.pid)
            } else {
                conn.program.clone()
            };

            let local_resolved = self.resolve_address(&conn.local);
            let remote_resolved = self.resolve_address(&conn.remote);

            let process_path = if conn.pid != "N/A" {
                self.get_process_path(&conn.pid)
            } else {
                "N/A".to_string()
            };

            let row_data = [
                prog_pid,
                conn.protocol.clone(),
                local_resolved,
                remote_resolved,
                conn.state.clone(),
                self.format_bytes(conn.tx_rate),
                self.format_bytes(conn.rx_rate),
                process_path,
            ];

            let mut row_widgets = self.row_widgets.lock().unwrap();
            let current_row = row;
            
            for (col, text) in row_data.iter().enumerate() {
                let label = Label::builder()
                    .label(text)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();

                // Enhanced styling based on column type
                match col {
                    0 => { // Process/PID
                        label.add_css_class("caption");
                        label.set_halign(Align::Start);
                    }
                    1 => { // Protocol
                        label.add_css_class("badge");
                        label.set_halign(Align::Center);
                        match conn.protocol.as_str() {
                            "tcp" => label.add_css_class("success"),
                            "udp" => label.add_css_class("warning"),
                            _ => label.add_css_class("dim-label"),
                        }
                    }
                    2 | 3 => { // Source/Destination
                        label.set_halign(Align::Start);
                        if col == 3 && (conn.rx_rate > 0 || conn.tx_rate > 0) {
                            label.add_css_class("accent");
                        }
                    }
                    4 => { // Status
                        label.set_halign(Align::Center);
                        match conn.state.as_str() {
                            "ESTABLISHED" => label.add_css_class("success"),
                            "LISTEN" => label.add_css_class("warning"),
                            "TIME_WAIT" => label.add_css_class("error"),
                            _ => label.add_css_class("dim-label"),
                        }
                    }
                    5 => { // TX Rate
                        label.set_halign(Align::End);
                        label.add_css_class("error");
                        if conn.tx_rate > 0 {
                            label.add_css_class("caption-heading");
                        }
                    }
                    6 => { // RX Rate
                        label.set_halign(Align::End);
                        label.add_css_class("accent");
                        if conn.rx_rate > 0 {
                            label.add_css_class("caption-heading");
                        }
                    }
                    7 => { // Path
                        label.add_css_class("caption");
                        label.add_css_class("dim-label");
                        label.set_halign(Align::Start);
                    }
                    _ => {
                        label.set_halign(Align::Start);
                    }
                }

                // Add click gesture for row selection
                let gesture = gtk::GestureClick::new();
                let selected_row = self.selected_row.clone();
                let row_widgets_clone = self.row_widgets.clone();
                let row_num = current_row;
                
                gesture.connect_pressed(move |_, _, _, _| {
                    // Update selected row
                    {
                        let mut selected = selected_row.lock().unwrap();
                        *selected = Some(row_num);
                    }
                    
                    // Update visual selection
                    let widgets = row_widgets_clone.lock().unwrap();
                    for (i, widget) in widgets.iter().enumerate() {
                        let widget_row = (i / 8) + 1; // 8 columns per row
                        if widget_row == row_num {
                            widget.add_css_class("selected");
                        } else {
                            widget.remove_css_class("selected");
                        }
                    }
                });
                
                label.add_controller(gesture);

                self.grid.attach(&label, col as i32, row as i32, 1, 1);
                row_widgets.push(label);
            }

            if conn.rx_rate > 0 || conn.tx_rate > 0 {
                active_connections += 1;
            }

            row += 1;
        }

        // Update status
        self.update_status(sorted_connections.len(), active_connections);
    }

    fn show_about_dialog(parent: &ApplicationWindow) {
        let about = AboutWindow::builder()
            .transient_for(parent)
            .modal(true)
            .application_name("Network Monitor")
            .application_icon("network-wired-symbolic")
            .version("1.0.0")
            .developer_name("Network Monitor Team")
            .copyright("© 2024 Network Monitor")
            .website("https://github.com/example/network-monitor")
            .license_type(gtk::License::Gpl30)
            .comments("A modern network connection monitoring tool with real-time updates and hostname resolution.")
            .build();

        about.present();
    }

    fn start_monitoring(self: &Arc<Self>) {
        // Initial update
        self.update_connections();

        // Set up periodic updates
        let monitor_clone = self.clone();
        timeout_add_seconds_local(3, move || {
            monitor_clone.update_connections();
            glib::ControlFlow::Continue
        });
    }
}

struct NetworkMonitorApp {
    app: Application,
    window: Arc<Mutex<Option<Arc<NetworkMonitorWindow>>>>,
}

impl NetworkMonitorApp {
    fn new() -> Self {
        let app = Application::builder()
            .application_id("com.example.NetworkMonitor")
            .build();

        // Set up style manager at application level
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::Default);

        let monitor = NetworkMonitorApp {
            app,
            window: Arc::new(Mutex::new(None)),
        };

        monitor.setup_actions();
        monitor
    }

    fn setup_actions(&self) {
        let refresh_action = gio::SimpleAction::new("refresh", None);
        let window = self.window.clone();
        refresh_action.connect_activate(move |_, _| {
            if let Some(window_guard) = window.lock().unwrap().as_ref() {
                window_guard.update_connections();
            }
        });
        self.app.add_action(&refresh_action);

        let about_action = gio::SimpleAction::new("about", None);
        let window = self.window.clone();
        about_action.connect_activate(move |_, _| {
            if let Some(window_guard) = window.lock().unwrap().as_ref() {
                NetworkMonitorWindow::show_about_dialog(&window_guard.window);
            }
        });
        self.app.add_action(&about_action);
    }

    fn run(&self) {
        let window = self.window.clone();
        self.app.connect_activate(move |app| {
            let monitor_window = NetworkMonitorWindow::new(app);
            monitor_window.window.present();
            
            let mut window_guard = window.lock().unwrap();
            *window_guard = Some(monitor_window);
        });

        self.app.run();
    }
}

fn main() {
    // Initialize GTK
    gtk::init().unwrap_or_else(|_| panic!("Failed to initialize GTK"));
    
    let app = NetworkMonitorApp::new();
    app.run();
}
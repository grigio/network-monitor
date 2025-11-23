use gtk4 as gtk;
use adw::{prelude::*, Application, ApplicationWindow, HeaderBar, AboutWindow};
use gtk::{Grid, Button, Label, ScrolledWindow, Orientation, Align, MenuButton, gio::Menu, Box as GtkBox};
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
    fixed_grid: Grid,
    scrollable_grid: Grid,
    resolve_toggle: gtk::CheckButton,
    header_buttons: Arc<Mutex<Vec<Button>>>,
    prev_io: Arc<Mutex<HashMap<String, ProcessIO>>>,
    resolution_cache: Arc<Mutex<HashMap<String, String>>>,
    resolution_pending: Arc<Mutex<std::collections::HashSet<String>>>,
    sort_column: Arc<Mutex<usize>>,
    sort_ascending: Arc<Mutex<bool>>,
    resolve_hosts: Arc<Mutex<bool>>,
    row_widgets: Arc<Mutex<Vec<Label>>>,
    fixed_row_widgets: Arc<Mutex<Vec<Label>>>,
    selected_row: Arc<Mutex<Option<usize>>>,
    connection_labels: Arc<Mutex<(Label, Label, Label, Label)>>,
}

impl NetworkMonitorWindow {
    fn new(app: &Application) -> Arc<Self> {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Network Monitor")
            .default_width(750)
            .default_height(650)
            .build();

        // Set up Adwaita style manager
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::Default);

        // Create fixed grid for Process column
        let fixed_grid = Grid::builder()
            .column_spacing(6)
            .row_spacing(1)
            .build();

        // Create scrollable grid for other columns
        let scrollable_grid = Grid::builder()
            .column_spacing(6)
            .row_spacing(1)
            .build();

        let resolve_toggle = gtk::CheckButton::builder()
            .label("Resolve Hostnames")
            .active(true)
            .build();

        // Create connection labels
        let total_label = Label::builder()
            .label("0 total connections")
            .halign(Align::Start)
            .build();
        total_label.add_css_class("caption");
        
        let active_label = Label::builder()
            .label("0 active connections")
            .halign(Align::Start)
            .build();
        active_label.add_css_class("caption");

        // Create data transfer labels
        let sent_label = Label::builder()
            .label("0 B sent")
            .halign(Align::Start)
            .build();
        sent_label.add_css_class("caption");
        
        let received_label = Label::builder()
            .label("0 B received")
            .halign(Align::Start)
            .build();
        received_label.add_css_class("caption");

        let monitor = Arc::new(NetworkMonitorWindow {
            window,
            fixed_grid,
            scrollable_grid,
            resolve_toggle,
            header_buttons: Arc::new(Mutex::new(Vec::new())),
            prev_io: Arc::new(Mutex::new(HashMap::new())),
            resolution_cache: Arc::new(Mutex::new(HashMap::new())),
            resolution_pending: Arc::new(Mutex::new(std::collections::HashSet::new())),
            sort_column: Arc::new(Mutex::new(6)),
            sort_ascending: Arc::new(Mutex::new(false)),
            resolve_hosts: Arc::new(Mutex::new(true)),
            row_widgets: Arc::new(Mutex::new(Vec::new())),
            fixed_row_widgets: Arc::new(Mutex::new(Vec::new())),
            selected_row: Arc::new(Mutex::new(None)),
            connection_labels: Arc::new(Mutex::new((total_label, active_label, sent_label, received_label))),
        });

        monitor.setup_grid();
        monitor.setup_ui();
        monitor.start_monitoring();
        monitor
    }

    fn setup_grid(self: &Arc<Self>) {
        // Create fixed Process column header
        let process_button = Button::builder()
            .label("Process(ID)")
            .build();
        process_button.add_css_class("heading");
        process_button.add_css_class("flat");
        process_button.add_css_class("fixed-column");
        
        // Connect click handler for sorting
        let monitor_clone = self.clone();
        process_button.connect_clicked(move |_| {
            let mut sort_col = monitor_clone.sort_column.lock().unwrap();
            let mut sort_asc = monitor_clone.sort_ascending.lock().unwrap();
            
            if *sort_col == 0 {
                *sort_asc = !*sort_asc;
            } else {
                *sort_col = 0;
                *sort_asc = true;
            }
            
            drop(sort_col);
            drop(sort_asc);
            
            let monitor_clone2 = monitor_clone.clone();
            glib::idle_add_local_once(move || {
                monitor_clone2.update_connections();
            });
        });
        
        self.fixed_grid.attach(&process_button, 0, 0, 1, 1);
        self.header_buttons.lock().unwrap().push(process_button);

        // Create scrollable column headers
        let scrollable_headers = [
            ("Protocol", 1),
            ("Source", 2),
            ("Destination", 3),
            ("Status", 4),
            ("TX", 5),
            ("RX", 6),
            ("Path", 7),
        ];

        for (text, col) in scrollable_headers {
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

            // Add resize gesture to header buttons
            let drag_gesture = gtk::GestureDrag::new();
            let button_clone = button.clone();
            
            drag_gesture.connect_drag_update(move |gesture, offset_x, _| {
                if let Some((start_x, _)) = gesture.start_point() {
                    let new_width = (start_x + offset_x).max(60.0) as i32;
                    
                    // Get current column width and update
                    let current_width = button_clone.width_request();
                    if new_width != current_width {
                        button_clone.set_width_request(new_width);
                    }
                }
            });
            
            button.add_controller(drag_gesture);
            
            // Adjust column index for scrollable grid (subtract 1 for fixed column)
            self.scrollable_grid.attach(&button, (col - 1) as i32, 0, 1, 1);
            
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
        menu_button.set_margin_end(4);
        let menu_model = self.create_menu_model();
        menu_button.set_menu_model(Some(&menu_model));
        header_bar.pack_end(&menu_button);

        main_box.append(&header_bar);

        // Create horizontal box for table layout
        let table_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        // Create fixed column container (Process column)
        let fixed_container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .build();
        fixed_container.add_css_class("fixed-container");
        fixed_container.append(&self.fixed_grid);
        
        // Create scrolled window for scrollable columns
        let scrolled = ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .min_content_height(300)
            .max_content_height(600)
            .build();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled.set_child(Some(&self.scrollable_grid));

        // Add both containers to table box
        table_box.append(&fixed_container);
        table_box.append(&scrolled);
        
        main_box.append(&table_box);



        // Add a separator line above the strip
        let separator = gtk::Separator::builder()
            .orientation(Orientation::Horizontal)
            .margin_start(12)
            .margin_end(12)
            .build();
        main_box.append(&separator);

        // Horizontal strip bottom control panel with two columns
        let control_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(0)
            .margin_top(0)
            .margin_bottom(0)
            .margin_start(0)
            .margin_end(0)
            .halign(Align::Fill)
            .valign(Align::Center)
            .height_request(32)
            .build();
        main_box.append(&control_box);

        // Left column: Network Monitor label and all connection info
        let left_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_start(12)
            .halign(Align::Start)
            .hexpand(true)
            .build();
        
        let monitor_label = Label::builder()
            .label("Network Monitor")
            .halign(Align::Start)
            .build();
        monitor_label.add_css_class("caption-heading");
        left_box.append(&monitor_label);

        // Single compact info group for all metrics
        let info_group = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(3)
            .build();
        info_group.add_css_class("info-group");
        
        // Total connections with icon
        let total_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .halign(Align::Start)
            .build();
        total_box.add_css_class("info-row");
        let total_icon = gtk::Image::from_icon_name("network-wired-symbolic");
        total_icon.add_css_class("caption");
        total_box.append(&total_icon);
        {
            let labels = self.connection_labels.lock().unwrap();
            total_box.append(&labels.0); // total connections
        }
        info_group.append(&total_box);
        
        // Active connections with icon
        let active_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .halign(Align::Start)
            .build();
        active_box.add_css_class("info-row");
        let active_icon = gtk::Image::from_icon_name("network-transmit-receive-symbolic");
        active_icon.add_css_class("caption");
        active_box.append(&active_icon);
        {
            let labels = self.connection_labels.lock().unwrap();
            active_box.append(&labels.1); // active connections
        }
        info_group.append(&active_box);

        // Data sent with icon
        let sent_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .halign(Align::Start)
            .build();
        sent_box.add_css_class("info-row");
        let sent_icon = gtk::Image::from_icon_name("go-up-symbolic");
        sent_icon.add_css_class("caption");
        sent_box.append(&sent_icon);
        {
            let labels = self.connection_labels.lock().unwrap();
            sent_box.append(&labels.2); // data sent
        }
        info_group.append(&sent_box);
        
        // Data received with icon
        let received_box = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .halign(Align::Start)
            .build();
        received_box.add_css_class("info-row");
        let received_icon = gtk::Image::from_icon_name("go-down-symbolic");
        received_icon.add_css_class("caption");
        received_box.append(&received_icon);
        {
            let labels = self.connection_labels.lock().unwrap();
            received_box.append(&labels.3); // data received
        }
        info_group.append(&received_box);
        
        left_box.append(&info_group);
        
        control_box.append(&left_box);

        // Right column: Host resolution checkbox
        let right_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_end(12)
            .margin_top(2)
            .halign(Align::End)
            .valign(Align::Center)
            .build();
        
        self.resolve_toggle.set_tooltip_text(Some("Toggle hostname resolution"));
        
        let resolution_cache = self.resolution_cache.clone();
        let resolve_hosts_field = self.resolve_hosts.clone();
        self.resolve_toggle.connect_toggled(move |button| {
            let resolve_hosts = button.is_active();
            *resolve_hosts_field.lock().unwrap() = resolve_hosts;
            if !resolve_hosts {
                resolution_cache.lock().unwrap().clear();
            }
        });
        
        right_box.append(&self.resolve_toggle);
        control_box.append(&right_box);

        // Update status
        self.update_status(0, 0, 0, 0);
    }

    fn apply_custom_css(&self) {
        let css_provider = gtk::CssProvider::new();
        let css = r#"
            .title {
                font-size: 1.2em;
                font-weight: 600;
                color: @headerbar_fg_color;
                margin: 0 8px;
                transition: all 150ms ease;
            }
            
            @define-color dark_bg_alpha rgba(0, 0, 0, 0.3);
            @define-color dark_hover_alpha rgba(255, 255, 255, 0.08);
            @define-color dark_selected_alpha rgba(255, 255, 255, 0.12);
            
            .card {
                border: none;
                border-radius: 4px;
                background: @view_bg_color;
                box-shadow: none;
            }
            
            .view {
                background: @view_bg_color;
                border: none;
                outline: none;
            }
            
            .toolbar {
                background: transparent;
                border-radius: 0px;
                padding: 4px 8px;
                border: none;
                box-shadow: none;
            }
            
            .inline-toolbar {
                background: transparent;
                border: none;
            }
            
            .heading {
                font-weight: 500;
                color: @headerbar_fg_color;
                font-size: 0.85em;
                text-transform: uppercase;
                letter-spacing: 0.2px;
                opacity: 0.9;
                cursor: ew-resize;
            }
            
            .fixed-column {
                background: @view_bg_color;
                border-right: 1px solid alpha(@borders, 0.3);
                position: relative;
                z-index: 10;
            }
            
            .fixed-container {
                background: @view_bg_color;
                border-right: 1px solid alpha(@borders, 0.3);
            }
            
            .badge {
                background: transparent;
                border-radius: 2px;
                padding: 2px 4px;
                font-size: 0.85em;
                border: none;
                color: @theme_fg_color;
            }
            
            .success {
                color: @success_color;
                background: transparent;
                border: none;
            }
            
            .warning {
                color: @warning_color;
                background: transparent;
                border: none;
            }
            
            .error {
                color: @error_color;
                background: transparent;
                border: none;
            }
            
            .accent {
                color: @accent_color;
                background: transparent;
                border: none;
            }
            
            .caption {
                font-size: 0.85em;
                opacity: 0.9;
            }
            
            .caption-heading {
                font-weight: 600;
                font-size: 1.0em;
                margin-bottom: 4px;
            }
            
            .info-group {
                background: alpha(@theme_bg_color, 0.03);
                border-radius: 4px;
                padding: 4px;
                margin-bottom: 3px;
            }
            
            .info-row {
                margin-bottom: 1px;
            }
            
            .dim-label {
                opacity: 0.65;
                font-style: italic;
            }
            
            grid {
                background: @view_bg_color;
                border-radius: 0px;
                border: none;
                padding: 4px;
                outline: none;
            }
            
            label {
                padding: 3px 5px;
                margin: 0px;
                border-radius: 2px;
                border: none;
                background: transparent;
                transition: all 120ms ease;
            }
            
            label:hover {
                background: alpha(@theme_bg_color, 0.3);
                box-shadow: none;
            }
            
            button {
                margin: 0px;
                border-radius: 2px;
                transition: all 120ms ease;
                min-width: 60px;
                min-height: 32px;
                border: none;
                background: transparent;
                border-right: 1px solid alpha(@borders, 0.3);
            }
            
            button:hover {
                background: alpha(@theme_bg_color, 0.2);
                box-shadow: none;
            }
            
            button:active {
                background: alpha(@theme_bg_color, 0.3);
                transform: none;
            }
            
            .flat {
                background: transparent;
                border-color: transparent;
            }
            
            .flat:hover {
                background: alpha(@theme_bg_color, 0.15);
            }
            
            .image-button {
                padding: 4px;
            }
            
            .toggle {
                padding: 3px 8px;
                font-weight: 400;
                border: none;
            }
            
            .toggle:checked {
                background: @accent_bg_color;
                color: @accent_fg_color;
                border-color: @accent_bg_color;
                box-shadow: 0 2px 6px alpha(@accent_bg_color, 0.3);
            }
            
            statuspage {
                padding: 24px;
            }
            
            statuspage > title {
                font-size: 1.1em;
                font-weight: 500;
                color: @headerbar_fg_color;
            }
            
            statuspage > description {
                font-size: 0.9em;
                opacity: 0.8;
            }
            
            .row-selected {
                background: alpha(@accent_bg_color, 0.1);
                border: none;
                color: @theme_fg_color;
                box-shadow: none;
            }
            
            .badge:hover {
                background: alpha(@theme_bg_color, 0.2);
                transform: none;
                box-shadow: none;
            }
            
            .success:hover {
                background: alpha(@theme_bg_color, 0.2);
            }
            
            .warning:hover {
                background: alpha(@theme_bg_color, 0.2);
            }
            
            .error:hover {
                background: alpha(@theme_bg_color, 0.2);
            }
            
            .accent:hover {
                background: alpha(@theme_bg_color, 0.2);
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

    fn format_bytes_total(&self, bytes_val: u64) -> String {
        let bytes_val = bytes_val as f64;
        
        // Always show in MB for consistency, with 2 decimal places
        if bytes_val < 1024.0 {
            format!("{:.1} B", bytes_val)
        } else if bytes_val < 1024.0 * 1024.0 {
            format!("{:.1} KB", bytes_val / 1024.0)
        } else {
            format!("{:.2} MB", bytes_val / (1024.0 * 1024.0))
        }
    }

    fn update_status(&self, total: usize, active: usize, total_sent: u64, total_received: u64) {
        // Update connection labels in bottom container
        {
            let labels = self.connection_labels.lock().unwrap();
            labels.0.set_text(&format!("{} total connections", total));
            labels.1.set_text(&format!("{} active connections", active));
            labels.2.set_text(&format!("Sent: {}", self.format_bytes_total(total_sent)));
            labels.3.set_text(&format!("Received: {}", self.format_bytes_total(total_received)));
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
                self.scrollable_grid.remove(widget);
            }
            row_widgets.clear();
        }
        
        {
            let mut fixed_row_widgets = self.fixed_row_widgets.lock().unwrap();
            for widget in fixed_row_widgets.iter() {
                self.fixed_grid.remove(widget);
            }
            fixed_row_widgets.clear();
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

        // Calculate total sent/received data
        let mut total_sent = 0u64;
        let mut total_received = 0u64;
        for io in current_io.values() {
            total_sent += io.tx;
            total_received += io.rx;
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

            // Create fixed Process column label
            let process_label = Label::builder()
                .label(&prog_pid)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            process_label.add_css_class("caption");
            process_label.add_css_class("badge");
            process_label.add_css_class("fixed-column");
            process_label.set_halign(Align::Start);

            // Add click gesture for row selection
            let gesture = gtk::GestureClick::new();
            let selected_row = self.selected_row.clone();
            let row_num = row;
            
            gesture.connect_pressed(move |_, _, _, _| {
                // Update selected row
                {
                    let mut selected = selected_row.lock().unwrap();
                    *selected = Some(row_num);
                }
            });
            
            process_label.add_controller(gesture);

            self.fixed_grid.attach(&process_label, 0, row as i32, 1, 1);
            self.fixed_row_widgets.lock().unwrap().push(process_label);

            // Create scrollable column data
            let scrollable_data = [
                conn.protocol.clone(),
                local_resolved,
                remote_resolved,
                conn.state.clone(),
                self.format_bytes(conn.tx_rate),
                self.format_bytes(conn.rx_rate),
                process_path,
            ];

            let mut row_widgets = self.row_widgets.lock().unwrap();
            
            for (col, text) in scrollable_data.iter().enumerate() {
                let label = Label::builder()
                    .label(text)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();

                // Simplified styling based on column type
                match col + 1 { // +1 because we skip the Process column
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
                        label.add_css_class("badge");
                        if col + 1 == 3 && (conn.rx_rate > 0 || conn.tx_rate > 0) {
                            label.add_css_class("accent");
                        }
                    }
                    4 => { // Status
                        label.set_halign(Align::Center);
                        label.add_css_class("badge");
                        match conn.state.as_str() {
                            "ESTABLISHED" => label.add_css_class("success"),
                            "LISTEN" => label.add_css_class("warning"),
                            "TIME_WAIT" => label.add_css_class("error"),
                            _ => label.add_css_class("dim-label"),
                        }
                    }
                    5 => { // TX Rate
                        label.set_halign(Align::End);
                        label.add_css_class("badge");
                        label.add_css_class("error");
                    }
                    6 => { // RX Rate
                        label.set_halign(Align::End);
                        label.add_css_class("badge");
                        label.add_css_class("accent");
                    }
                    7 => { // Path
                        label.add_css_class("caption");
                        label.add_css_class("dim-label");
                        label.add_css_class("badge");
                        label.set_halign(Align::Start);
                    }
                    _ => {
                        label.set_halign(Align::Start);
                        label.add_css_class("badge");
                    }
                }

                // Add click gesture for row selection
                let gesture = gtk::GestureClick::new();
                let selected_row = self.selected_row.clone();
                let row_num = row;
                
                gesture.connect_pressed(move |_, _, _, _| {
                    // Update selected row
                    {
                        let mut selected = selected_row.lock().unwrap();
                        *selected = Some(row_num);
                    }
                });
                
                label.add_controller(gesture);

                self.scrollable_grid.attach(&label, col as i32, row as i32, 1, 1);
                row_widgets.push(label);
            }

            if conn.rx_rate > 0 || conn.tx_rate > 0 {
                active_connections += 1;
            }

            row += 1;
        }

        // Update status
        self.update_status(sorted_connections.len(), active_connections, total_sent, total_received);
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
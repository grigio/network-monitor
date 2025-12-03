use adw::{prelude::*, AboutWindow, Application, ApplicationWindow, HeaderBar};
use gio::Menu;
use glib::timeout_add_seconds_local;
use gtk::{
    Align, Box as GtkBox, Grid, Label, MenuButton, Orientation, PopoverMenu, ScrolledWindow,
};
use gtk4 as gtk;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::models::{Connection, ProcessIO};
use crate::services::{AddressResolver, NetworkService};
use crate::utils::formatter::Formatter;

/// Main application window
pub struct NetworkMonitorWindow {
    pub window: ApplicationWindow,
    header_grid: Grid,
    content_grid: Grid,
    resolve_toggle: gtk::CheckButton,
    header_labels: Rc<RefCell<Vec<Label>>>,
    prev_io: Arc<Mutex<HashMap<String, ProcessIO>>>,
    resolver: AddressResolver,
    network_service: NetworkService,
    sort_column: Rc<RefCell<usize>>,
    sort_ascending: Rc<RefCell<bool>>,
    row_widgets: Rc<RefCell<Vec<Label>>>,
    selected_row: Rc<RefCell<Option<usize>>>,
    connection_labels: Rc<RefCell<(Label, Label, Label, Label)>>,
    column_widths: Rc<RefCell<Vec<i32>>>,
    active_popovers: Rc<RefCell<Vec<PopoverMenu>>>,
}

impl NetworkMonitorWindow {
    pub fn new(app: &Application) -> Rc<Self> {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Network Monitor")
            .default_width(500)
            .default_height(700)
            .resizable(true)
            .build();

        // WM class is handled by application ID in GTK4

        // Add CSS class for window width control
        window.add_css_class("main-window");

        // Set up Adwaita style manager
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::Default);

        // Create separate grids for sticky header and scrollable content
        let header_grid = Grid::builder()
            .column_spacing(0)
            .row_spacing(0)
            .halign(Align::Fill)
            .hexpand(true)
            .build();

        let content_grid = Grid::builder()
            .column_spacing(0)
            .row_spacing(0)
            .halign(Align::Fill)
            .hexpand(true)
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

        let monitor = Rc::new(NetworkMonitorWindow {
            window,
            header_grid,
            content_grid,
            resolve_toggle,
            header_labels: Rc::new(RefCell::new(Vec::new())),
            prev_io: Arc::new(Mutex::new(HashMap::new())),
            resolver: AddressResolver::new(true),
            network_service: NetworkService::new(),
            sort_column: Rc::new(RefCell::new(6)),
            sort_ascending: Rc::new(RefCell::new(false)),
            row_widgets: Rc::new(RefCell::new(Vec::new())),
            selected_row: Rc::new(RefCell::new(None)),
            connection_labels: Rc::new(RefCell::new((
                total_label,
                active_label,
                sent_label,
                received_label,
            ))),
            column_widths: Rc::new(RefCell::new(vec![0; 8])), // 8 columns
            active_popovers: Rc::new(RefCell::new(Vec::new())),
        });

        monitor.setup_grid();
        monitor.setup_ui();
        monitor.setup_column_sync();
        monitor.setup_close_handler();
        monitor.start_monitoring();
        monitor
    }

    fn setup_grid(self: &Rc<Self>) {
        // Create all column headers as clickable labels
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
            let label = Label::builder().label(text).build();
            label.add_css_class("table-header");

            // Set alignment and width constraints for header labels
            match col {
                0 => {
                    // Process(ID) - left aligned with specific width
                    label.set_halign(Align::Start);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                    label.add_css_class("column-process");
                }
                1 => {
                    // Protocol - left aligned with specific width
                    label.set_halign(Align::Start);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                    label.add_css_class("column-protocol");
                }
                2 | 3 => {
                    // Source/Destination - left aligned with specific width
                    label.set_halign(Align::Start);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                    label.add_css_class("column-address");
                }
                4 => {
                    // Status - left aligned with specific width
                    label.set_halign(Align::Start);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                    label.add_css_class("column-status");
                }
                5 | 6 => {
                    // TX/RX - left aligned with specific width
                    label.set_halign(Align::Start);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                    label.add_css_class("column-rate");
                }
                7 => {
                    // Path - left aligned with specific width
                    label.set_halign(Align::Start);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                    label.add_css_class("column-path");
                }
                _ => {
                    label.set_halign(Align::Start);
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                }
            }

            // Connect click handler for sorting
            let monitor_clone = self.clone();
            let col_index = col;

            let gesture = gtk::GestureClick::new();
            gesture.connect_pressed(move |_, _, _, _| {
                let mut sort_col = monitor_clone.sort_column.borrow_mut();
                let mut sort_asc = monitor_clone.sort_ascending.borrow_mut();

                if *sort_col == col_index {
                    *sort_asc = !*sort_asc;
                } else {
                    *sort_col = col_index;
                    *sort_asc = false; // First click should be descending
                }

                drop(sort_col);
                drop(sort_asc);

                let monitor_clone2 = monitor_clone.clone();
                glib::idle_add_local_once(move || {
                    monitor_clone2.update_connections();
                    monitor_clone2.update_header_labels();
                });
            });

            label.add_controller(gesture);

            self.header_grid.attach(&label, col as i32, 0, 1, 1);

            // Store header labels for styling
            self.header_labels.borrow_mut().push(label);
        }
    }

    fn setup_ui(self: &Rc<Self>) {
        // Apply custom CSS
        self.apply_custom_css();

        // Create responsive main box
        let main_box = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .hexpand(true) // Allow horizontal expansion
            .halign(Align::Fill) // Fill available space
            .build();

        self.window.set_content(Some(&main_box));

        // Enhanced header bar with better styling
        let title_label = Label::builder().label("Network Monitor").build();
        title_label.add_css_class("title");

        let header_bar = HeaderBar::builder().title_widget(&title_label).build();
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

        // Create responsive table container
        let table_container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .hexpand(true) // Allow horizontal expansion
            .halign(Align::Fill) // Fill available space
            .build();
        table_container.add_css_class("table-container");
        table_container.add_css_class("responsive-table");

        // Create header container with sticky behavior
        let header_container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .build();
        header_container.add_css_class("header-container");
        header_container.add_css_class("sticky-header");
        header_container.append(&self.header_grid);

        // Create scrolled window for content with responsive behavior
        let scrolled = ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .halign(Align::Fill)
            .height_request(400)
            .min_content_width(450)
            .build();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled.add_css_class("table-container");
        scrolled.add_css_class("responsive-table");
        scrolled.set_child(Some(&self.content_grid));

        // Improved horizontal scrolling synchronization
        let header_grid_clone1 = self.header_grid.clone();
        let header_grid_clone2 = self.header_grid.clone();
        let scrolled_clone = scrolled.clone();

        // Get horizontal adjustment for smooth scrolling sync
        let hadjustment = scrolled.hadjustment();

        // Sync header position with horizontal scroll
        hadjustment.connect_value_notify(move |hadj| {
            let scroll_value = hadj.value();
            header_grid_clone1.set_margin_start(-(scroll_value.round() as i32));
        });

        // Handle edge cases for overscroll
        scrolled.connect_edge_overshot(move |_, pos| {
            if pos == gtk::PositionType::Left || pos == gtk::PositionType::Right {
                let hadjustment = scrolled_clone.hadjustment();
                let scroll_value = hadjustment.value();
                header_grid_clone2.set_margin_start(-(scroll_value.round() as i32));
            }
        });

        // Ensure header and content have same width constraints
        self.content_grid.set_size_request(450, -1);
        self.header_grid.set_size_request(450, -1);

        table_container.append(&header_container);
        table_container.append(&scrolled);

        main_box.append(&table_container);

        // Update header labels after UI is rendered
        let monitor_clone = self.clone();
        glib::idle_add_local_once(move || {
            monitor_clone.update_header_labels();
        });

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
            let labels = self.connection_labels.borrow();
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
            let labels = self.connection_labels.borrow();
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
            let labels = self.connection_labels.borrow();
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
            let labels = self.connection_labels.borrow();
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

        self.resolve_toggle
            .set_tooltip_text(Some("Toggle hostname resolution"));

        let resolver = self.resolver.clone();
        self.resolve_toggle.connect_toggled(move |button| {
            let resolve_hosts = button.is_active();
            resolver.set_resolve_hosts(resolve_hosts);
        });

        right_box.append(&self.resolve_toggle);
        control_box.append(&right_box);

        // Update status
        self.update_status(0, 0, 0, 0);
    }

    fn apply_custom_css(&self) {
        let css_provider = gtk::CssProvider::new();
        let css = include_str!("styles.css");
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

        // Create theme submenu
        let theme_submenu = Menu::new();
        theme_submenu.append(Some("Light"), Some("app.theme_light"));
        theme_submenu.append(Some("Dark"), Some("app.theme_dark"));
        theme_submenu.append(Some("Auto"), Some("app.theme_auto"));

        menu.append_submenu(Some("Theme"), &theme_submenu);
        menu.append(Some("About"), Some("app.about"));
        menu.append(Some("Quit"), Some("app.quit"));
        menu
    }

    pub fn update_connections(self: &Rc<Self>) {
        // Clean up any active popovers before clearing widgets
        {
            let mut popovers = self.active_popovers.borrow_mut();
            for popover in popovers.drain(..) {
                popover.unparent();
            }
        }

        // Clear existing grid content (except headers)
        {
            let mut row_widgets = self.row_widgets.borrow_mut();
            for widget in row_widgets.iter() {
                self.content_grid.remove(widget);
            }
            row_widgets.clear();
        }

        // Get connections
        let connections = self.network_service.get_connections();

        // Update I/O data for rate calculations
        let prev_io = self.prev_io.lock().unwrap().clone();
        let (updated_connections, current_io) = self
            .network_service
            .update_connection_rates(connections, &prev_io);

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
            .filter(|conn| self.resolver.resolve_address(&conn.remote) != "LOCALHOST")
            .collect();

        // Sort connections
        let sorted_connections = self.sort_connections(filtered_connections);

        let mut active_connections = 0;
        let mut row = 1; // Start from row 1 (row 0 is headers)

        for conn in &sorted_connections {
            // Format display values
            let prog_pid = conn.get_process_display();

            let local_resolved = self.resolver.resolve_address(&conn.local);
            let remote_resolved = self.resolver.resolve_address(&conn.remote);

            let process_path = conn.command.clone();

            let mut row_widgets = self.row_widgets.borrow_mut();

            // Process each column separately to avoid lifetime issues
            let columns = [
                prog_pid,
                conn.protocol.clone(),
                local_resolved,
                remote_resolved,
                conn.state.clone(),
                Formatter::format_bytes(conn.tx_rate),
                Formatter::format_bytes(conn.rx_rate),
                process_path,
            ];

            for (col, text) in columns.iter().enumerate() {
                let text_for_closures = text.clone(); // Clone for closures
                let label = Label::builder()
                    .label(text)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .xalign(0.0) // Default to left alignment
                    .build();

                // Enhanced styling with column-specific classes and alignment
                match col {
                    0 => {
                        // Process(ID)
                        label.add_css_class("caption");
                        label.add_css_class("table-cell");
                        label.add_css_class("column-process");
                        label.set_halign(Align::Start);
                        label.set_xalign(0.0); // Left align
                        label.set_hexpand(true);
                    }
                    1 => {
                        // Protocol
                        label.add_css_class("table-cell");
                        label.add_css_class("column-protocol");
                        label.set_halign(Align::Start);
                        label.set_xalign(0.0); // Left align
                        label.set_hexpand(true);
                        match conn.protocol.as_str() {
                            "tcp" => label.add_css_class("success"),
                            "udp" => label.add_css_class("warning"),
                            _ => label.add_css_class("dim-label"),
                        }
                    }
                    2 | 3 => {
                        // Source/Destination
                        label.add_css_class("table-cell");
                        label.add_css_class("column-address");
                        label.set_halign(Align::Start);
                        label.set_xalign(0.0); // Left align
                        label.set_hexpand(true);
                        if col == 3 && (conn.rx_rate > 0 || conn.tx_rate > 0) {
                            label.add_css_class("accent");
                        }
                    }
                    4 => {
                        // Status
                        label.add_css_class("table-cell");
                        label.add_css_class("column-status");
                        label.set_halign(Align::Start);
                        label.set_xalign(0.0); // Left align
                        label.set_hexpand(true);
                        match conn.state.as_str() {
                            "ESTABLISHED" => label.add_css_class("success"),
                            "LISTEN" => label.add_css_class("warning"),
                            "TIME_WAIT" => label.add_css_class("error"),
                            _ => label.add_css_class("dim-label"),
                        }
                    }
                    5 => {
                        // TX Rate
                        label.add_css_class("table-cell");
                        label.add_css_class("column-rate");
                        label.add_css_class("error");
                        label.set_halign(Align::End);
                        label.set_xalign(1.0); // Right align
                        label.set_hexpand(true);
                    }
                    6 => {
                        // RX Rate
                        label.add_css_class("table-cell");
                        label.add_css_class("column-rate");
                        label.add_css_class("accent");
                        label.set_halign(Align::End);
                        label.set_xalign(1.0); // Right align
                        label.set_hexpand(true);
                    }
                    7 => {
                        // Path
                        label.add_css_class("caption");
                        label.add_css_class("dim-label");
                        label.add_css_class("table-cell");
                        label.add_css_class("column-path");
                        label.set_halign(Align::Start);
                        label.set_xalign(0.0); // Left align
                        label.set_hexpand(true);
                    }
                    _ => {
                        label.add_css_class("table-cell");
                        label.set_halign(Align::Start);
                        label.set_xalign(0.0); // Left align
                        label.set_hexpand(true);
                    }
                }

                // Add click gesture for row selection
                let gesture = gtk::GestureClick::new();
                let selected_row = self.selected_row.clone();
                let row_num = row;

                gesture.connect_pressed(move |_, _, _, _| {
                    // Update selected row
                    {
                        let mut selected = selected_row.borrow_mut();
                        *selected = Some(row_num);
                    }
                });

                label.add_controller(gesture);

                // Add right-click gesture for context menu
                let right_click_gesture = gtk::GestureClick::new();
                right_click_gesture.set_button(3); // Right mouse button;

                let text_for_right_click = text_for_closures.clone();
                let active_popovers = self.active_popovers.clone();
                right_click_gesture.connect_pressed(move |gesture, _, x, y| {
                    let copy_text = text_for_right_click.clone();

                    // Copy immediately on right-click
                    if let Some(display) = gtk::gdk::Display::default() {
                        let clipboard = display.clipboard();
                        clipboard.set_text(&copy_text);
                    }

                    // Show simple notification menu
                    let menu = PopoverMenu::builder().build();

                    // Create menu model
                    let menu_model = Menu::new();
                    menu_model.append(Some("Copied!"), None);
                    menu.set_menu_model(Some(&menu_model));

                    // Position the menu at cursor
                    if let Some(parent) = gesture.widget() {
                        menu.set_parent(&parent);
                        let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                        menu.set_pointing_to(Some(&rect));

                        // Track this popover for cleanup
                        let active_popovers_clone = active_popovers.clone();
                        let menu_clone = menu.clone();
                        active_popovers_clone.borrow_mut().push(menu_clone.clone());

                        // Auto-hide after 1 second
                        let menu_for_timeout = menu.clone();
                        let active_popovers_for_timeout = active_popovers.clone();
                        glib::timeout_add_seconds_local_once(1, move || {
                            menu_for_timeout.unparent();
                            // Remove from active popovers
                            let mut popovers = active_popovers_for_timeout.borrow_mut();
                            popovers.retain(|p| !p.eq(&menu_for_timeout));
                        });

                        menu.popup();
                    }
                });

                // Add keyboard shortcut for Ctrl+C
                let key_controller = gtk::EventControllerKey::new();
                let text_for_keyboard = text_for_closures.clone();
                key_controller.connect_key_pressed(move |_, key, _, modifier| {
                    if key == gtk::gdk::Key::c && modifier == gtk::gdk::ModifierType::CONTROL_MASK {
                        if let Some(display) = gtk::gdk::Display::default() {
                            let clipboard = display.clipboard();
                            clipboard.set_text(&text_for_keyboard);
                        }
                        return glib::Propagation::Stop;
                    }
                    glib::Propagation::Proceed
                });

                label.add_controller(key_controller);

                label.add_controller(right_click_gesture);

                self.content_grid
                    .attach(&label, col as i32, row as i32, 1, 1);
                row_widgets.push(label);
            }

            if conn.is_active() {
                active_connections += 1;
            }

            row += 1;
        }

        // Update status
        self.update_status(
            sorted_connections.len(),
            active_connections,
            total_sent,
            total_received,
        );

        // Sync column widths after updating content
        let header_grid = self.header_grid.clone();
        let content_grid = self.content_grid.clone();
        let column_widths = self.column_widths.clone();

        glib::idle_add_local_once(move || {
            Self::sync_column_widths(&header_grid, &content_grid, &column_widths);
        });
    }

    fn update_status(&self, total: usize, active: usize, total_sent: u64, total_received: u64) {
        // Update connection labels in bottom container
        {
            let labels = self.connection_labels.borrow();
            labels.0.set_text(&format!("{total} total connections"));
            labels.1.set_text(&format!("{active} active connections"));
            labels.2.set_text(&format!(
                "Sent: {}",
                Formatter::format_bytes_total(total_sent)
            ));
            labels.3.set_text(&format!(
                "Received: {}",
                Formatter::format_bytes_total(total_received)
            ));
        }
    }

    fn sort_connections(&self, connections: Vec<Connection>) -> Vec<Connection> {
        if connections.is_empty() {
            return connections;
        }

        let sort_column = *self.sort_column.borrow();
        let sort_ascending = *self.sort_ascending.borrow();

        let mut sorted_connections = connections;

        sorted_connections.sort_by(|a, b| {
            let comparison = match sort_column {
                0 => a.get_process_display().cmp(&b.get_process_display()),
                1 => a.protocol.cmp(&b.protocol),
                2 => self
                    .resolver
                    .resolve_address(&a.local)
                    .cmp(&self.resolver.resolve_address(&b.local)),
                3 => self
                    .resolver
                    .resolve_address(&a.remote)
                    .cmp(&self.resolver.resolve_address(&b.remote)),
                4 => a.state.cmp(&b.state),
                5 => a.tx_rate.cmp(&b.tx_rate),
                6 => a.rx_rate.cmp(&b.rx_rate),
                7 => a.command.cmp(&b.command),
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

    pub fn show_about_dialog(parent: &ApplicationWindow) {
        let about = AboutWindow::builder()
            .transient_for(parent)
            .modal(true)
            .application_name("Network Monitor")
            .application_icon("network-wired-symbolic")
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("Network Monitor Team")
            .copyright("© 2024 Network Monitor")
            .website("https://github.com/grigio/network-monitor")
            .license_type(gtk::License::Gpl30)
            .comments("A modern network connection monitoring tool with real-time updates and hostname resolution.")
            .build();

        about.present();
    }

    fn update_header_labels(&self) {
        let sort_column = *self.sort_column.borrow();
        let sort_ascending = *self.sort_ascending.borrow();
        let header_labels = self.header_labels.borrow();

        // Define base labels for each column
        let base_labels = [
            "Process(ID)",
            "Protocol",
            "Source",
            "Destination",
            "Status",
            "TX",
            "RX",
            "Path",
        ];

        for (index, label) in header_labels.iter().enumerate() {
            let base_label = base_labels.get(index).unwrap_or(&"");
            let triangle = if index == sort_column {
                if sort_ascending {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            };
            label.set_text(&format!("{base_label}{triangle}"));
        }
    }

    fn setup_column_sync(self: &Rc<Self>) {
        // Set up column width synchronization
        let header_grid1 = self.header_grid.clone();
        let content_grid1 = self.content_grid.clone();
        let column_widths1 = self.column_widths.clone();

        // Connect to window size changes
        let window_clone = self.window.clone();
        window_clone.connect_default_width_notify(move |_| {
            // Schedule column width update after layout is complete
            let header_grid = header_grid1.clone();
            let content_grid = content_grid1.clone();
            let column_widths = column_widths1.clone();

            glib::idle_add_local_once(move || {
                Self::sync_column_widths(&header_grid, &content_grid, &column_widths);
            });
        });

        // Initial sync
        let header_grid2 = self.header_grid.clone();
        let content_grid2 = self.content_grid.clone();
        let column_widths2 = self.column_widths.clone();
        glib::idle_add_local_once(move || {
            Self::sync_column_widths(&header_grid2, &content_grid2, &column_widths2);
        });
    }

    fn sync_column_widths(
        header_grid: &Grid,
        content_grid: &Grid,
        column_widths: &Rc<RefCell<Vec<i32>>>,
    ) {
        // Get the first row of content to measure column widths
        let mut max_widths = vec![0; 8];

        // Only sync if we have content to measure
        if let Some(first_child) = content_grid.first_child() {
            let mut current_child = Some(first_child);
            let mut col = 0;

            while let Some(child) = current_child {
                if child.downcast_ref::<Label>().is_some() {
                    // Use a reasonable default width with padding
                    let width = 120; // Default width for now
                    if col < max_widths.len() {
                        max_widths[col] = max_widths[col].max(width);
                    }
                    col += 1;
                }

                current_child = child.next_sibling();
                if col >= 8 {
                    break;
                }
            }
        }

        // Apply minimum widths with reasonable constraints
        let min_widths = [120, 40, 120, 120, 50, 60, 60, 150];
        let max_widths_limit = [300, 60, 200, 200, 80, 80, 80, 400];
        for i in 0..8 {
            max_widths[i] = max_widths[i].max(min_widths[i]).min(max_widths_limit[i]);
        }

        // Only update if widths actually changed to avoid unnecessary layout passes
        let current_widths = column_widths.borrow();
        let widths_changed = current_widths.len() != max_widths.len()
            || current_widths
                .iter()
                .zip(max_widths.iter())
                .any(|(curr, new)| curr != new);
        drop(current_widths);

        if widths_changed {
            // Update header column widths more gently
            let header_labels = header_grid.observe_children();
            for i in 0..header_labels.n_items().min(8) {
                let idx = i as usize;
                if let Some(child) = header_labels.item(i) {
                    if let Some(label) = child.downcast_ref::<Label>() {
                        // Only set width request if it's different
                        if label.width_request() != max_widths[idx] {
                            label.set_width_request(max_widths[idx]);
                        }
                    }
                }
            }

            // Store widths for future reference
            *column_widths.borrow_mut() = max_widths;
        }
    }

    fn setup_close_handler(self: &Rc<Self>) {
        // Handle window close event to properly quit the application
        self.window.connect_close_request(move |window| {
            // Quit the application directly
            if let Some(app) = window.application() {
                app.quit();
            }

            // Return true to indicate we've handled the close request
            glib::Propagation::Stop
        });
    }

    fn start_monitoring(self: &Rc<Self>) {
        // Initial update
        self.update_connections();
        self.update_header_labels();

        // Set up periodic updates
        let monitor_clone = self.clone();
        timeout_add_seconds_local(3, move || {
            monitor_clone.update_connections();
            glib::ControlFlow::Continue
        });
    }
}

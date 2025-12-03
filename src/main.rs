use adw::{prelude::*, Application};
use gio::SimpleAction;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

// Import modules
mod error;
mod error_tests;
mod models;
mod services;
mod ui;
mod utils;

use ui::NetworkMonitorWindow;

/// Main application structure
struct NetworkMonitorApp {
    app: Application,
    window: Rc<RefCell<Option<Rc<NetworkMonitorWindow>>>>,
}

impl NetworkMonitorApp {
    fn new() -> Self {
        let app = Application::builder()
            .application_id("org.grigio.NetworkMonitor")
            .flags(gio::ApplicationFlags::HANDLES_OPEN)
            .build();

        // Set up style manager at application level
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::Default);

        let monitor = NetworkMonitorApp {
            app,
            window: Rc::new(RefCell::new(None)),
        };

        monitor.setup_actions();
        monitor
    }

    fn setup_actions(&self) {
        let refresh_action = SimpleAction::new("refresh", None);
        let window = self.window.clone();
        refresh_action.connect_activate(move |_, _| {
            if let Some(window_guard) = window.borrow().as_ref() {
                window_guard.update_connections();
            }
        });
        self.app.add_action(&refresh_action);

        let about_action = SimpleAction::new("about", None);
        let window = self.window.clone();
        about_action.connect_activate(move |_, _| {
            if let Some(window_guard) = window.borrow().as_ref() {
                NetworkMonitorWindow::show_about_dialog(&window_guard.window);
            }
        });
        self.app.add_action(&about_action);

        // Theme switching actions
        let theme_light_action = SimpleAction::new("theme_light", None);
        let style_manager = adw::StyleManager::default();
        theme_light_action.connect_activate(move |_, _| {
            style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
        });
        self.app.add_action(&theme_light_action);

        let theme_dark_action = SimpleAction::new("theme_dark", None);
        let style_manager = adw::StyleManager::default();
        theme_dark_action.connect_activate(move |_, _| {
            style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
        });
        self.app.add_action(&theme_dark_action);

        let theme_auto_action = SimpleAction::new("theme_auto", None);
        let style_manager = adw::StyleManager::default();
        theme_auto_action.connect_activate(move |_, _| {
            style_manager.set_color_scheme(adw::ColorScheme::Default);
        });
        self.app.add_action(&theme_auto_action);

        // Copy action for table cells
        let copy_action = SimpleAction::new("copy", None);
        self.app.add_action(&copy_action);

        // Quit action
        let quit_action = SimpleAction::new("quit", None);
        let app = self.app.clone();
        quit_action.connect_activate(move |_, _| {
            app.quit();
        });
        self.app.add_action(&quit_action);
    }

    fn run(&self) {
        let window = self.window.clone();
        let window_for_shutdown = window.clone();

        // Handle primary instance activation
        self.app.connect_activate(move |app| {
            let mut window_guard = window.borrow_mut();

            if window_guard.is_none() {
                // First activation - create window
                let monitor_window = NetworkMonitorWindow::new(app);
                monitor_window.window.present();
                *window_guard = Some(monitor_window);
            } else {
                // Already running - bring existing window to front
                if let Some(existing_window) = window_guard.as_ref() {
                    existing_window.window.present();
                }
            }
        });

        // Handle shutdown to properly clean up resources
        self.app.connect_shutdown(move |_| {
            // Clean up window reference
            *window_for_shutdown.borrow_mut() = None;
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

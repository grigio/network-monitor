use adw::{prelude::*, Application};
use gio::ActionEntry;
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
        // About action
        let action_about = ActionEntry::builder("about")
            .activate(move |_, _, _| {
                // This will be handled by the window when it's created
            })
            .build();

        // Theme switching actions
        let action_theme_light = ActionEntry::builder("theme_light")
            .activate(move |_, _, _| {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
            })
            .build();

        let action_theme_dark = ActionEntry::builder("theme_dark")
            .activate(move |_, _, _| {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
            })
            .build();

        let action_theme_auto = ActionEntry::builder("theme_auto")
            .activate(move |_, _, _| {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::Default);
            })
            .build();

        self.app.add_action_entries([
            action_about,
            action_theme_light,
            action_theme_dark,
            action_theme_auto,
        ]);
    }

    fn run(&self) {
        let window = self.window.clone();
        let window_for_shutdown = window.clone();

        // Set keyboard accelerators
        self.app.set_accels_for_action("app.about", &["F1"]);
        self.app
            .set_accels_for_action("app.theme_light", &["<Ctrl>L"]);
        self.app
            .set_accels_for_action("app.theme_dark", &["<Ctrl>D"]);
        self.app
            .set_accels_for_action("app.theme_auto", &["<Ctrl>T"]);

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
    // Initialize GTK with proper error handling
    if let Err(e) = gtk::init() {
        eprintln!("Failed to initialize GTK: {}", e);
        eprintln!("This usually means the X11/Wayland display is not available.");
        eprintln!("Try running in a proper desktop environment or check your display settings.");
        std::process::exit(1);
    }

    let app = NetworkMonitorApp::new();
    app.run();
}

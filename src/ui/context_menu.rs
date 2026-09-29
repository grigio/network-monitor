//! Right-click context menu: copy the cell/command and kill the process that
//! owns the connection.
//!
//! The menu is built on demand for the row that was right-clicked and its
//! target is kept in [`NetworkMonitorWindow::context_target`] so the actions
//! (registered in [`NetworkMonitorWindow::setup_context_actions`]) can act on
//! it without the closures having to own the window.

use adw::{prelude::*, ResponseAppearance};
use gio::{ActionEntry, Menu};
use gtk::PopoverMenu;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::Connection;
use crate::services::process_ops::{kill_process, parse_pid, KillSignal};

use super::window::NetworkMonitorWindow;

impl NetworkMonitorWindow {
    /// Register the window actions used by the context menu and by the
    /// `Delete` keyboard shortcut.
    pub(super) fn setup_context_actions(self: &Rc<Self>) {
        let owner = self.clone();
        let action_copy_cell = ActionEntry::builder("copy-cell")
            .activate(move |_, _, _| {
                let text = owner.context_cell_text.borrow().clone();
                if text.is_empty() {
                    owner.show_toast("Nothing to copy");
                    return;
                }
                copy_to_clipboard(&text);
                owner.show_toast("Copied to clipboard");
            })
            .build();

        let owner = self.clone();
        let action_copy_command = ActionEntry::builder("copy-command")
            .activate(move |_, _, _| {
                let command = owner
                    .context_target
                    .borrow()
                    .as_ref()
                    .map(|conn| conn.command.clone())
                    .unwrap_or_default();
                if command.is_empty() {
                    owner.show_toast("No command available for this row");
                    return;
                }
                copy_to_clipboard(&command);
                owner.show_toast("Command copied");
            })
            .build();

        let owner = self.clone();
        let action_kill = ActionEntry::builder("kill-process")
            .activate(move |_, _, _| {
                let target = owner.context_target.borrow().clone();
                match target {
                    Some(conn) => owner.present_kill_dialog(&conn),
                    None => owner.show_toast("No row selected"),
                }
            })
            .build();

        let owner = self.clone();
        let action_kill_selected = ActionEntry::builder("kill-selected")
            .activate(move |_, _, _| match owner.selected_connection() {
                Some(conn) => owner.present_kill_dialog(&conn),
                None => owner.show_toast("No row selected"),
            })
            .build();

        self.window.add_action_entries([
            action_copy_cell,
            action_copy_command,
            action_kill,
            action_kill_selected,
        ]);

        if let Some(app) = self.window.application() {
            app.set_accels_for_action("win.kill-selected", &["Delete"]);
        }
    }

    /// Show the context menu for `target` at `(x, y)` on `parent`.
    ///
    /// Refreshes are paused (see `update_connections`) until this menu is
    /// dismissed, so the rows on screen stay in sync with the target.
    pub(super) fn show_row_context_menu(
        menu_slot: &Rc<RefCell<Option<PopoverMenu>>>,
        parent: &impl IsA<gtk::Widget>,
        x: f64,
        y: f64,
        target: &Connection,
    ) {
        // Replace a menu that is still lingering from a previous right-click
        if let Some(old_menu) = menu_slot.borrow_mut().take() {
            old_menu.unparent();
        }

        let menu_model = Menu::new();

        let copy_section = Menu::new();
        copy_section.append(Some("Copy Value"), Some("win.copy-cell"));
        copy_section.append(Some("Copy Command"), Some("win.copy-command"));
        menu_model.append_section(None, &copy_section);

        // Placeholder rows and connections without a usable pid cannot be killed
        if parse_pid(&target.pid).is_ok() {
            let kill_section = Menu::new();
            kill_section.append(
                Some(&format!(
                    "Kill \u{201c}{}\u{201d} ({})",
                    target.program, target.pid
                )),
                Some("win.kill-process"),
            );
            menu_model.append_section(None, &kill_section);
        }

        let menu = PopoverMenu::builder().build();
        menu.set_menu_model(Some(&menu_model));
        menu.set_parent(parent);
        menu.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

        // Release the refresh gate as soon as the menu is dismissed and drop
        // the popover so it does not stay attached to a recycled row label.
        let menu_slot_for_closed = menu_slot.clone();
        menu.connect_closed(move |closed_menu| {
            *menu_slot_for_closed.borrow_mut() = None;
            let menu_to_unparent = closed_menu.clone();
            glib::idle_add_local_once(move || {
                if menu_to_unparent.parent().is_some() {
                    menu_to_unparent.unparent();
                }
            });
        });

        *menu_slot.borrow_mut() = Some(menu.clone());
        menu.popup();
    }

    /// Ask the user how to signal the process behind `target`.
    fn present_kill_dialog(self: &Rc<Self>, target: &Connection) {
        if let Err(e) = parse_pid(&target.pid) {
            self.show_toast(&format!("Cannot kill this row: {e}"));
            return;
        }

        let dialog = adw::AlertDialog::builder()
            .heading(format!("Kill \u{201c}{}\u{201d}?", target.program))
            .body(format!(
                "PID {} owns every connection this process has. \
                 Terminate asks it to shut down gracefully, force kill ends it immediately.",
                target.pid
            ))
            .default_response("cancel")
            .close_response("cancel")
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("term", KillSignal::Term.description());
        dialog.add_response("force", KillSignal::Force.description());
        dialog.set_response_appearance("term", ResponseAppearance::Suggested);
        dialog.set_response_appearance("force", ResponseAppearance::Destructive);

        let owner = self.clone();
        let target = target.clone();
        dialog.connect_response(None, move |_dialog, response| {
            let signal = match response {
                "term" => KillSignal::Term,
                "force" => KillSignal::Force,
                _ => return,
            };
            owner.perform_kill(&target, signal);
        });

        dialog.present(Some(&self.window));
    }

    /// Send the signal and report the outcome as a toast.
    fn perform_kill(self: &Rc<Self>, target: &Connection, signal: KillSignal) {
        match kill_process(&target.pid, signal) {
            Ok(()) => {
                self.show_toast(&format!(
                    "{} sent to {} ({})",
                    signal.label(),
                    target.program,
                    target.pid
                ));
                // Reflect the result immediately instead of waiting for the
                // periodic refresh.
                self.update_connections();
            }
            Err(e) => {
                self.show_toast(&format!("Could not kill {}: {}", target.program, e));
            }
        }
    }

    /// The connection currently selected in the table, if any.
    fn selected_connection(&self) -> Option<Connection> {
        let row_num: usize = (*self.selected_row.borrow())?;
        let rows = self.displayed_rows.borrow();
        let row = rows.get(row_num.checked_sub(1)?)?;
        if row.placeholder {
            return None;
        }
        Some(row.conn.clone())
    }

    /// Transient feedback for actions that would otherwise be silent.
    fn show_toast(&self, message: &str) {
        self.toast_overlay.add_toast(adw::Toast::new(message));
    }
}

/// Copy `text` to the system clipboard, if a display is available.
pub(super) fn copy_to_clipboard(text: &str) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(text);
    } else {
        eprintln!("Warning: Could not access clipboard - display not available");
    }
}

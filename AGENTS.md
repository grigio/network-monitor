# Network Monitor - Rust + GTK4 + TUI Guide

## Technology Stack
- **Rust 2021 Edition**: Systems programming with memory safety
- **GTK4**: Modern cross-platform GUI framework with v4_14 features
- **Libadwaita**: GNOME-style UI components with v1_5 features
- **Ratatui**: Terminal User Interface framework (beta version for latest features)
- **Crossterm**: Cross-platform terminal handling with event streaming
- **GLib/GIO**: GObject system and I/O operations
- **Serde**: Serialization framework with JSON support
- **ThisError**: Error handling with context
- **Native socket parsing**: Direct `/proc/net` filesystem access
- **Inode-based process mapping**: Socket-to-process identification

## Project Structure
```
src/
├── main.rs          # GTK4 application entry point
├── tui_main.rs      # TUI application entry point
├── ui/              # GTK4 UI components and widgets
├── models/          # Data structures and state (shared)
├── services/        # Business logic and system calls (shared)
│   ├── network.rs       # Native socket parsing and process mapping
│   ├── process_cache.rs # Process information caching for performance
│   ├── resolver.rs     # Address resolution utilities
│   └── tests.rs        # Comprehensive unit tests for core logic
├── utils/           # Helper functions (shared)
│   ├── formatter.rs    # Consolidated formatting utilities for GTK/TUI
│   ├── parsing.rs      # Hex parsing and common operations with error context
│   ├── recovery.rs     # Graceful degradation and circuit breaker patterns
│   └── mod.rs         # Module exports and shared utilities
├── error.rs         # Custom error types with thiserror
└── error_tests.rs    # Error handling tests
```

## Essential Dependencies

See `Cargo.toml` for exact dependency versions and features.

### Core Dependencies
- **gtk4**: Modern GTK4 bindings with v4_14 features
- **libadwaita**: GNOME-style UI components with v1_5 features  
- **glib**: GLib bindings for GObject system
- **gio**: GIO bindings for I/O operations
- **serde**: Serialization framework with derive support
- **serde_json**: JSON serialization support
- **thiserror**: Error handling with context
- **crossterm**: Terminal handling with event streaming
- **ratatui**: Terminal User Interface framework with crossterm backend

### Dependency Status
- All dependencies are current and actively maintained
- Using beta version of ratatui for latest features
- Security audit passed with no critical vulnerabilities
- License compliance verified with cargo-deny
- Duplicate dependency warnings present (itertools 0.13.0/0.14.0) but non-critical

## Common Patterns

### Actions and Menus (GTK4 Guidelines)
**IMPORTANT**: Always follow the official GTK4 actions and menus documentation at https://gtk-rs.org/gtk4-rs/stable/latest/book/actions.html#menus for implementing menus and actions.

```rust
use gio::ActionEntry;

// Use ActionEntry builder pattern for actions
let action_about = ActionEntry::builder("about")
    .activate(move |window: &ApplicationWindow, _, _| {
        // Action implementation
    })
    .build();

// Add actions using add_action_entries method
window.add_action_entries([action_about]);

// Set keyboard accelerators
app.set_accels_for_action("win.about", &["F1"]);

// Create menu model with proper action references
let menu = Menu::new();
let section = Menu::new();
section.append(Some("About"), Some("win.about"));
menu.append_section(Some("Help"), &section);
```

**Key Requirements**:
- Use `ActionEntry::builder()` instead of `SimpleAction::new()`
- Use `add_action_entries()` instead of individual `add_action()` calls
- Organize actions with proper prefixes: `app.*` for application-level, `win.*` for window-level
- Set keyboard accelerators with `set_accels_for_action()`
- Reference actions in menus using full action name (e.g., `"win.about"`)

**Menu Styling (Adwaita)**:
```css
/* Fix double borders and transparency */
popover {
    border: none;
    box-shadow: 0 4px 12px alpha(black, 0.12);
    background: var(--popover-bg-color);
}

popover contents {
    border: none;
    box-shadow: none;
    background: transparent;
}

menubutton > popover {
    border: none;
    box-shadow: 0 4px 12px alpha(black, 0.12);
}
```

### Async Operations with GTK
```rust
use glib::clone;
use tokio::runtime::Runtime;

// Spawn async task from GTK callback
let rt = Runtime::new().unwrap();
glib::spawn_future_local(async move {
    rt.spawn(async {
        // Async system calls here
    });
});
```

### State Management
- Use `Rc<RefCell<T>>` for shared mutable state
- Consider `once_cell::sync::Lazy` for global state
- Implement `Default` and `Clone` for complex types

## Development Commands
```bash
# Development build (GTK4)
cargo run

# Development build (TUI)
cargo run --bin nmt

# Release build (both binaries)
cargo build --release

# Build specific binary
cargo build --bin network-monitor
cargo build --bin nmt

# Local installation (no sudo required)
./scripts/install.sh

# System-wide installation (requires sudo)
sudo ./scripts/install.sh

# Local uninstallation
./scripts/uninstall.sh

# System-wide uninstallation (requires sudo)
sudo ./scripts/uninstall.sh

# Format code
cargo fmt

# Run lints
cargo clippy -- -D warnings

# Run tests
cargo test

# Run tests with error handling coverage
cargo test error_tests

# Run security audit (requires cargo-audit)
cargo audit

# Run dependency checks
cargo deny check

# Run clippy lints
cargo clippy -- -D warnings
```

## Critical Pitfalls
1. **Thread Safety**: GTK is not thread-safe - use `glib::spawn_future_local()`
2. **Memory Leaks**: Avoid circular references in callbacks
3. **Async Integration**: Properly bridge Tokio and GTK main loops
4. **Resource Management**: Clean up system resources in `Drop` implementations
5. **Process Mapping**: Use inode-based mapping for accurate socket-to-process identification
6. **File System Access**: Handle `/proc` filesystem access errors gracefully with proper Result types
7. **WM Class Matching**: Ensure `StartupWMClass` in desktop file matches `window.set_class_name()` for GNOME dock pinning
8. **Terminal Compatibility**: TUI requires proper terminal environment - avoid running in limited IDE terminals
9. **Code Sharing**: Maintain shared modules (models, services, utils) to avoid duplication between GTK and TUI versions
10. **Error Handling**: Use custom `NetworkMonitorError` types instead of `.unwrap()` calls for robust error recovery
11. **Performance**: Utilize process caching and layout caching to reduce system calls and improve responsiveness
12. **Security**: Regular dependency audits and automated security checks in CI pipeline
13. **Code Quality**: Comprehensive unit tests, error recovery patterns, and consistent formatting utilities

## GTK4 + Adwaita Styling Guidelines

### CSS Variables and Colors
- **Always use modern CSS variables** instead of deprecated GTK3 named colors
- **Selection colors**: Use `var(--accent-bg-color)` and `var(--accent-fg-color)` for hover/selection states
- **Deprecated colors**: Avoid `@theme_selected_bg_color`, `@theme_selected_fg_color` - use CSS variables instead
- **Color mixing**: Use `color-mix(in srgb, var(--accent-bg-color) 85%, black)` for active states
- **Proper opacity**: Use `alpha(var(--accent-bg-color), 0.15)` for subtle hover effects

### Menu and Popover Styling
- **Menu items**: Use `modelbutton` selector with proper hover/active/checked states
- **Hover effect**: Background color change only, text color stays consistent with theme
- **Popover styling**: Use `var(--popover-bg-color)` and `var(--popover-fg-color)` for menu containers
- **Menu button**: Apply `.image-button` class with subtle hover effects using accent colors

### Adwaita Style Classes
- **Buttons**: Use `.suggested-action`, `.destructive-action`, `.flat`, `.circular`, `.pill` as appropriate
- **Typography**: Use `.title-1` through `.title-4`, `.heading`, `.body`, `.caption`, `.monospace` classes
- **Colors**: Apply `.accent`, `.success`, `.warning`, `.error` classes for semantic coloring
- **Layout**: Use `.card`, `.boxed-list`, `.navigation-sidebar` for container styling

### Theme Compatibility
- **Light/Dark support**: Always test styling in both light and dark themes
- **High contrast**: Use CSS variables that automatically adapt to high contrast mode
- **System integration**: Follow GNOME HIG for consistent user experience
- **Responsive design**: Ensure UI works well with different window sizes and scaling

### Common Styling Patterns
```css
/* Correct menu item hover */
modelbutton:hover {
    background: var(--accent-bg-color);
    color: var(--accent-fg-color);
}

/* Correct button hover */
.image-button:hover {
    background: alpha(var(--accent-bg-color), 0.15);
}

/* Use semantic color classes */
.success { color: var(--success-color); }
.warning { color: var(--warning-color); }
.error { color: var(--error-color); }
```

### Resources
- [Adwaita Style Classes](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/style-classes.html)
- [CSS Variables Reference](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/css-variables.html)
- [GNOME HIG](https://developer.gnome.org/hig/)

## Implementation Details

### Network Connection Monitoring
The application uses native Rust libraries to monitor network connections:

1. **Direct `/proc/net` parsing**: Reads from `/proc/net/tcp`, `/proc/net/tcp6`, `/proc/net/udp`, and `/proc/net/udp6`
2. **Inode-based process mapping**: Maps socket inodes to processes via `/proc/*/fd` for accurate PID identification
3. **Process information extraction**: Gets process names from `/proc/[pid]/status` and command lines from `/proc/[pid]/cmdline`
4. **I/O statistics**: Reads real-time I/O data from `/proc/[pid]/io` for TX/RX rate calculations

### Key Advantages Over External Tools
- **No external dependencies**: Doesn't rely on `ss` or other system utilities
- **More reliable**: Not affected by changes in external tool output format
- **Better performance**: Direct file system access instead of spawning processes
- **Accurate mapping**: Inode-based process mapping provides precise socket-to-process relationships

## Performance Tips
- Use `glib::idle_add_once()` for non-critical UI updates
- Batch multiple UI changes in single closure
- Cache expensive system information
- Use async/await for blocking operations
- Use native socket parsing instead of external commands for better performance
- Implement efficient inode-to-process mapping to avoid scanning entire `/proc` tree unnecessarily
- For TUI: Use terminal escape sequences efficiently and limit refresh rate to avoid flickering
- Share core logic between GTK and TUI versions to maintain consistency and reduce maintenance
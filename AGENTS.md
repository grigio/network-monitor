# Network Monitor - Rust + GTK4 + TUI Guide

## Technology Stack
- **Rust 2021 Edition**: Systems programming with memory safety
- **GTK4**: Modern cross-platform GUI framework with v4_14 features
- **Libadwaita**: GNOME-style UI components with v1_5 features
- **Ratatui**: Terminal User Interface framework (beta version for latest features)
- **Crossterm**: Cross-platform terminal handling with event streaming
- **eBPF + Aya**: Kernel-level connection tracing with kprobes for real-time events
- **Process I/O**: Direct `/proc/[pid]/io` reading for TX/RX rate calculation
- **PID from eBPF**: Direct PID capture at probe point via `bpf_get_current_pid_tgid()`
- **Capability-based permissions**: Requires `CAP_BPF + CAP_NET_ADMIN + CAP_PERFMON`

## Project Structure
```
src/
├── main.rs                      # GTK4 application entry point
├── tui_main.rs                  # TUI application entry point
├── ui/                          # GTK4 UI components and widgets
├── models/                      # Data structures and state (shared)
├── services/                    # Business logic and system calls (shared)
│   ├── connection_monitor.rs    # ConnectionMonitor trait + factory
│   ├── ebpf_monitor.rs          # eBPF-based connection monitor
│   └── resolver.rs              # DNS resolution
├── utils/                       # Helper functions (shared)
├── error.rs                     # Custom error types with thiserror
└── error_tests.rs               # Error handling tests

network-monitor-common/          # Shared types between kernel and userspace
├── Cargo.toml
└── src/lib.rs                   # EbpfEvent, TcpConnectEvent, etc.

network-monitor-ebpf/            # eBPF kernel programs (requires nightly)
├── Cargo.toml
├── .cargo/config.toml           # bpfel-unknown-none target config
└── src/lib.rs                   # kprobe/kretprobe definitions

build.rs                         # Optional eBPF program compilation
```

## Monitor Architecture

The app uses a **single eBPF-based backend** with no fallback — eBPF is required:

```
EbpfMonitor  ──  eBPF kprobe tracing
```

At startup, `detect_best_monitor()` creates an `EbpfMonitor`. If eBPF is unavailable (no permissions or old kernel), the app exits with a setup message.

### eBPF Architecture (when enabled)

```
KERNEL SPACE:
  kprobe/tcp_v4_connect      ──┐
  kprobe/tcp_v6_connect      ──┤──> PerfEventArray ──┐
  kprobe/tcp_close           ──┤                      │
  kretprobe/inet_csk_accept  ──┘                      │
                                                      ▼
USER SPACE:
  aya::Ebpf (load + attach) ──> PerfEventArray reader ──> connection state
                                 +
  /proc/*/io  ──> TX/RX rate calculation

  On startup: /proc/net fallback for rehydration (existing connections)
```

Key differences from `/proc/net` approach:
- **Event-driven**: Captures every connect/accept/close, no polling
- **Direct PID**: `bpf_get_current_pid_tgid()` at probe point, no inode scanning
- **Real-time**: Sub-millisecond event delivery
- **No missed connections**: Short-lived connections are never lost between polls

## Performance Optimizations

### UI Performance Enhancements
- **Debouncing**: Implemented debouncing for UI updates (200ms delay, 500ms minimum interval) to prevent excessive updates and reduce CPU usage
- **Virtualization**: Added row virtualization for large datasets (>100 connections) to show only first/last 50 rows with placeholders, significantly reducing widget count
- **Column Width Caching**: Implemented caching system for column widths with 10px threshold changes to avoid repeated layout calculations
- **Layout Optimization**: Reduced sampling rate for column width measurement (1 every 5 rows) and added conservative text width estimation

### TUI Performance
- **Layout Caching**: Added layout cache system with validation based on width and connection count changes
- **Skip Rendering**: Implemented skip rendering when no significant changes detected to improve TUI responsiveness
- **FPS Monitoring**: Added FPS monitoring with warnings for low performance

### Backend Performance
- **eBPF backend**: ~1-3% CPU, event-driven, no polling overhead
- **Process cache**: 5-second TTL for inode-to-PID mapping, lazy refresh

### Critical Implementation Notes
- **RefCell Management**: Careful RefCell borrowing patterns implemented to avoid runtime panics. Multiple mutable RefCell accesses are properly scoped to prevent borrowing conflicts.
- **Memory Management**: Widget reuse implemented to minimize memory allocation and improve performance during frequent updates.
- **Thread Safety**: All UI updates properly scheduled through GTK's main loop using `glib::spawn_future_local()` and `glib::idle_add_local_once()`.

## Common Patterns

### ConnectionMonitor Trait
```rust
pub trait ConnectionMonitor: Send {
    fn get_connections(&self) -> Result<Vec<Connection>>;
    fn get_process_io(&self, pid: &str) -> ProcessIO;
    fn update_connection_rates(
        &self,
        connections: Vec<Connection>,
        prev_io: &HashMap<String, ProcessIO>,
    ) -> Result<(Vec<Connection>, HashMap<String, ProcessIO>)>;
    fn name(&self) -> &'static str;
}
```

### Frontend Usage
```rust
use crate::services::{detect_best_monitor, ConnectionMonitor};

// Create the best available monitor at startup
let monitor: Box<dyn ConnectionMonitor> = detect_best_monitor();

// Monitor selection is transparent to the frontend
let connections = monitor.get_connections()?;
let (updated, current_io) = monitor.update_connection_rates(connections, &prev_io)?;
```

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
# Build and run (eBPF backend requires nightly + bpf-linker)
cargo run                    # GTK4 version
cargo run --bin nmt          # TUI version
cargo build --release        # Release build

# Prerequisites for eBPF build
rustup toolchain install nightly
cargo install bpf-linker

# Build eBPF programs manually (if build.rs fails)
cargo +nightly build --release --target bpfel-unknown-none -Z build-std=core \
  -p network-monitor-ebpf

# Code quality
cargo fmt                    # Format code
cargo clippy -- -D warnings  # Lint with strict warnings
cargo test                   # Run tests

# Dependency management
cargo update                 # Update dependencies
cargo outdated               # Check for outdated dependencies
cargo audit                  # Security audit
cargo deny check             # License and dependency checks

# Installation
./scripts/install.sh         # Local install
sudo ./scripts/install.sh    # System-wide install

# before commit run
cargo outdated
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build
```

## Critical Pitfalls
1. **Thread Safety**: GTK is not thread-safe - use `glib::spawn_future_local()`
2. **Memory Leaks**: Avoid circular references in callbacks
3. **Async Integration**: Properly bridge Tokio and GTK main loops
4. **Resource Management**: Clean up system resources in `Drop` implementations
5. **Process Mapping**: Direct PID from eBPF events via `bpf_get_current_pid_tgid()`
6. **File System Access**: Handle `/proc` filesystem access errors gracefully with proper Result types
7. **WM Class Matching**: Ensure `StartupWMClass` in desktop file matches `window.set_class_name()` for GNOME dock pinning
8. **Terminal Compatibility**: TUI requires proper terminal environment - avoid running in limited IDE terminals
9. **Code Sharing**: Maintain shared modules (models, services, utils) to avoid duplication between GTK and TUI versions
10. **Unwrap Usage**: Only use `.unwrap()` or `.expect()` in tests, examples, or when you can **prove** the operation cannot fail (e.g., parsing hardcoded constants, valid regex patterns). For all production code handling external input, user data, or system resources, use proper error handling with `?` operator and custom error types.
11. **Error Handling**: Use custom `NetworkMonitorError` types instead of `.unwrap()` calls for robust error recovery
12. **Performance**: Utilize process caching and layout caching to reduce system calls and improve responsiveness
13. **Security**: Regular dependency audits and automated security checks in CI pipeline
14. **Code Quality**: Comprehensive unit tests, error recovery patterns, and consistent formatting utilities

### eBPF-Specific Pitfalls
- **eBPF requires nightly Rust**: The `network-monitor-ebpf` crate uses `#![no_std]` and must be compiled with `rustup toolchain install nightly` and `bpf-linker`
- **Kernel requirements**: eBPF kprobes need Linux 4.1+. CO-RE/BTF support needs 5.2+. Granular capabilities need 5.8+.
- **Permissions**: eBPF requires `CAP_BPF` + `CAP_NET_ADMIN` + `CAP_PERFMON` (or `CAP_SYS_ADMIN` on kernels <5.8). The app exits with a setup message when these are missing.
- **Capability pitfalls running without sudo**:
  - **Filesystem `nosuid`**: Most Linux filesystems (including `ext4` with `nosuid` mount flag, and `/tmp`) strip file capabilities. Check with `mount | grep nosuid`. Workaround: copy binary to a filesystem without `nosuid`, or bind-mount with `suid`.
    ```
    sudo mkdir -p /tmp/nosuid && sudo mount --bind /tmp /tmp/nosuid && sudo mount -o remount,suid /tmp/nosuid
    cp target/debug/nmt /tmp/nosuid/ && sudo setcap cap_bpf,cap_net_admin,cap_perfmon+ep /tmp/nosuid/nmt && /tmp/nosuid/nmt
    ```
  - **`kernel.unprivileged_bpf_disabled=2`**: The kernel may block eBPF for non-root entirely via `sysctl kernel.unprivileged_bpf_disabled`. If set to `2`, even file capabilities won't help (kernel ignores them). Fix:
    ```bash
    sudo sysctl -w kernel.unprivileged_bpf_disabled=0
    sudo sh -c 'echo "kernel.unprivileged_bpf_disabled=0" >> /etc/sysctl.d/99-bpf.conf'
    ```
  - **Wrong capability bit numbers in code**: The capability check reads `/proc/self/status` `CapEff` field and compares bit positions. The bit positions **must match the kernel's `<linux/capability.h>` definitions**:
    | Capability | Bit | Value |
    |---|---|---|
    | `CAP_NET_ADMIN` | 12 | `1 << 12` |
    | `CAP_SYS_ADMIN` | 21 | `1 << 21` |
    | `CAP_PERFMON` | 38 | `1 << 38` |
    | `CAP_BPF` | 39 | `1 << 39` |
    These are defined in `src/services/ebpf_monitor.rs:check_ebpf_availability()`.
- **The eBPF build is required**: The `build.rs` script compiles eBPF programs at build time. Nightly Rust and `bpf-linker` are mandatory prerequisites.
- **eBPF kernel programs use `#[repr(C)]` types**: Shared types in `network-monitor-common` must remain `no_std` compatible for the eBPF side.

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
The application supports two backends for monitoring network connections:

#### Default Backend: /proc/net Parsing (ProcMonitor)
1. **Direct `/proc/net` parsing**: Reads from `/proc/net/tcp`, `/proc/net/tcp6`, `/proc/net/udp`, and `/proc/net/udp6`
2. **Inode-based process mapping**: Maps socket inodes to processes via `/proc/*/fd` for accurate PID identification
3. **Process information extraction**: Gets process names from `/proc/[pid]/status` and command lines from `/proc/[pid]/cmdline`
4. **I/O statistics**: Reads real-time I/O data from `/proc/[pid]/io` for TX/RX rate calculations

#### Optional Backend: eBPF Tracing (EbpfMonitor)
1. **Kernel-level kprobes**: Traces `tcp_v4_connect`, `tcp_v6_connect`, `tcp_close`, and `inet_csk_accept`
2. **Direct process identification**: Uses `bpf_get_current_pid_tgid()` at the probe point (no inode scanning)
3. **PerfEventArray**: Events delivered to userspace via eBPF ring buffer
4. **Rehydration on startup**: Falls back to `/proc/net` to discover pre-existing connections
5. **TX/RX rates**: Still reads `/proc/[pid]/io` for rate calculation (shared logic)

### Key Advantages Over External Tools
- **No external dependencies**: Doesn't rely on `ss` or other system utilities
- **More reliable**: Not affected by changes in external tool output format
- **Better performance**: Direct file system access instead of spawning processes
- **Dual strategy**: Falls back gracefully from eBPF to /proc/net
- **Event-driven** (eBPF): Catches short-lived connections missed by polling

## Performance Tips
- Use `glib::idle_add_once()` for non-critical UI updates
- Batch multiple UI changes in single closure
- Cache expensive system information
- Use async/await for blocking operations
- Use native socket parsing instead of external commands for better performance
- Implement efficient inode-to-process mapping to avoid scanning entire `/proc` tree unnecessarily
- For TUI: Use terminal escape sequences efficiently and limit refresh rate to avoid flickering
- Share core logic between GTK and TUI versions to maintain consistency and reduce maintenance
- Keep dependencies updated with `cargo update` and check outdated packages with `cargo outdated`
- Enable the `ebpf` feature for event-driven monitoring; the app degrades gracefully if unavailable
- Monitor backend selection: run with `RUST_LOG=info` to see which monitor backend is active

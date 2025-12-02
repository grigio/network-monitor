# Network Monitor - Rust + GTK4 Guide

## Technology Stack
- **Rust 2021 Edition**: Systems programming with memory safety
- **GTK4**: Modern cross-platform GUI framework  
- **Libadwaita**: GNOME-style UI components
- **Tokio**: Async runtime for concurrent operations
- **Native socket parsing**: Direct `/proc/net` filesystem access
- **Inode-based process mapping**: Socket-to-process identification

## Project Structure
```
src/
├── main.rs          # Application entry point
├── ui/              # UI components and widgets
├── models/          # Data structures and state
├── services/        # Business logic and system calls
│   ├── network.rs   # Native socket parsing and process mapping
│   └── resolver.rs # Address resolution utilities
└── utils/           # Helper functions
```

## Essential Dependencies
See `Cargo.toml` for exact dependency versions and features.

## Common Patterns

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
# Development build
cargo run

# Release build
cargo build --release

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
```

## Critical Pitfalls
1. **Thread Safety**: GTK is not thread-safe - use `glib::spawn_future_local()`
2. **Memory Leaks**: Avoid circular references in callbacks
3. **Async Integration**: Properly bridge Tokio and GTK main loops
4. **Resource Management**: Clean up system resources in `Drop` implementations
5. **Process Mapping**: Use inode-based mapping for accurate socket-to-process identification
6. **File System Access**: Handle `/proc` filesystem access errors gracefully
7. **WM Class Matching**: Ensure `StartupWMClass` in desktop file matches `window.set_class_name()` for GNOME dock pinning

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
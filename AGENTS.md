# Network Monitor - Rust + GTK4 Guide

## Technology Stack
- **Rust 2021 Edition**: Systems programming with memory safety
- **GTK4**: Modern cross-platform GUI framework  
- **Libadwaita**: GNOME-style UI components
- **Tokio**: Async runtime for concurrent operations

## Project Structure
```
src/
├── main.rs          # Application entry point
├── ui/              # UI components and widgets
├── models/          # Data structures and state
├── services/        # Business logic and system calls
└── utils/           # Helper functions
```

## Essential Dependencies
```toml
[dependencies]
gtk4 = { version = "0.9", features = ["v4_14"] }
adw = { version = "0.7", features = ["v1_5"], package = "libadwaita" }
tokio = { version = "1.0", features = ["full"] }
```

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

## Performance Tips
- Use `glib::idle_add_once()` for non-critical UI updates
- Batch multiple UI changes in single closure
- Cache expensive system information
- Use async/await for blocking operations
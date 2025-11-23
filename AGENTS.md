# Network Monitor - Agent Documentation

## Project Overview

Network Monitor is a Rust + GTK4 application for real-time network connection monitoring. This project demonstrates modern Rust GUI development using the GNOME ecosystem.

## Technology Stack

- **Rust 2021 Edition**: Systems programming language with memory safety
- **GTK4**: Modern cross-platform GUI framework
- **Libadwaita**: GNOME-style UI components for consistent design
- **Tokio**: Async runtime for concurrent operations
- **System Integration**: Direct `/proc` filesystem access for performance

## Architecture Decisions

### Why Rust + GTK4?
- **Performance**: Rust's zero-cost abstractions and memory safety ideal for system monitoring
- **Modern UI**: GTK4 provides hardware-accelerated rendering and modern widgets
- **Cross-platform**: Single codebase targets Linux, Windows, macOS
- **Ecosystem**: Mature bindings (gtk4-rs) and strong GNOME integration

### Async Architecture
- Uses Tokio for non-blocking I/O operations
- Prevents UI freezing during system calls
- Enables concurrent network monitoring and UI updates

## Best Practices for Rust + GTK4 Projects

### Project Structure
```
src/
├── main.rs          # Application entry point
├── ui/              # UI components and widgets
├── models/          # Data structures and state
├── services/        # Business logic and system calls
└── utils/           # Helper functions
```

### Dependencies Management
```toml
[dependencies]
gtk4 = { version = "0.9", features = ["v4_14"] }
adw = { version = "0.7", features = ["v1_5"], package = "libadwaita" }
tokio = { version = "1.0", features = ["full"] }
```

### Common Patterns

#### 1. Application Setup
```rust
use gtk4::prelude::*;
use adw::prelude::*;

fn main() {
    let app = gtk4::Application::new(Some("com.example.app"), Default::default());
    app.connect_activate(build_ui);
    app.run();
}
```

#### 2. Async Operations with GTK
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

#### 3. State Management
- Use `Rc<RefCell<T>>` for shared mutable state
- Consider `once_cell::sync::Lazy` for global state
- Implement `Default` and `Clone` for complex types

#### 4. Error Handling
- Use `anyhow` for application-level errors
- Implement proper error propagation with `?` operator
- Show user-friendly error dialogs

### Performance Considerations

#### 1. UI Updates
- Use `glib::idle_add_once()` for non-critical updates
- Batch multiple UI changes in single closure
- Avoid expensive operations in UI callbacks

#### 2. System Calls
- Cache expensive system information
- Use async/await for blocking operations
- Implement rate limiting for frequent updates

#### 3. Memory Management
- Prefer references over cloning when possible
- Use `String` over `&str` for stored data
- Implement `Drop` for cleanup when needed

### Development Workflow

#### 1. Building
```bash
# Development build with debugging
cargo run

# Optimized release build
cargo build --release

# Run tests
cargo test
```

#### 2. Linting and Formatting
```bash
# Format code
cargo fmt

# Run clippy for lints
cargo clippy -- -D warnings

# Check for security issues
cargo audit
```

#### 3. Testing
- Unit tests for business logic
- Integration tests for UI components
- Property-based testing for complex algorithms

### Platform-Specific Notes

#### Linux
- Requires GTK4 and Libadwaita development packages
- Best integration with GNOME desktop
- Full feature support

#### Other Platforms
- Windows: Requires GTK4 MSVC packages
- macOS: Requires Homebrew GTK4 installation
- Some features may be platform-dependent

## Common Pitfalls

1. **Thread Safety**: GTK is not thread-safe - use `glib::spawn_future_local()`
2. **Memory Leaks**: Avoid circular references in callbacks
3. **Async Integration**: Properly bridge Tokio and GTK main loops
4. **Resource Management**: Clean up system resources in `Drop` implementations

## Resources

- [GTK4 Rust Documentation](https://gtk-rs.org/gtk4-rs/)
- [Libadwaita Documentation](https://gnome.pages.gitlab.gnome.org/libadwaita/)
- [Tokio Documentation](https://tokio.rs/tokio/tutorial)
- [Rust Book](https://doc.rust-lang.org/book/)
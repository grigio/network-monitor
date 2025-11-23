# Network Monitor

A real-time network connection monitoring tool built with Rust and GTK4, displaying active connections with live I/O statistics in a modern graphical interface.

## Features

- **Real-time monitoring**: Continuously monitors active network connections
- **I/O statistics**: Shows live upload/download rates for each connection
- **Process identification**: Displays the program and PID associated with each connection
- **Modern GTK4 UI**: Clean, responsive graphical interface with Libadwaita styling
- **Address resolution**: Simplifies common addresses (localhost, any, mDNS)
- **Connection filtering**: Filters out localhost connections for cleaner output

## Requirements

- Rust 1.70+ (2021 edition)
- GTK4 development libraries
- Libadwaita development libraries
- Linux system with `/proc` filesystem

### Installation on Ubuntu/Debian:
```bash
sudo apt update
sudo apt install libgtk-4-dev libadwaita-1-dev
```

### Installation on Fedora:
```bash
sudo dnf install gtk4-devel libadwaita-devel
```

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd network-monitor
```

2. Build and run:
```bash
cargo run
```

Or build in release mode:
```bash
cargo build --release
./target/release/network-monitor
```

## Usage

Launch the network monitor application:
```bash
cargo run
```

The application will open a GTK4 window displaying:
- **Protocol**: TCP/UDP protocol
- **State**: Connection state (ESTABLISHED, LISTEN, etc.)
- **Local Address**: Local endpoint (resolved to readable format)
- **Remote Address**: Remote endpoint (resolved to readable format)
- **Program(PID)**: Process name and PID
- **RX Rate**: Download rate
- **TX Rate**: Upload rate

### Address Resolution

Common addresses are simplified for readability:
- `0.0.0.0:*` or `*:*` → `ANY`
- `127.0.0.1:*` or `[::1]:*` → `LOCALHOST`
- `224.0.0.251:*` → `MDNS`

## How It Works

1. Uses `ss -tulnape` to get active connections with process information
2. Reads `/proc/[pid]/io` for real-time I/O statistics
3. Calculates rates by comparing I/O between updates
4. Updates GTK4 interface every second with current connection state

## Architecture

- **GTK4**: Modern cross-platform GUI framework
- **Libadwaita**: GNOME-style UI components
- **Tokio**: Async runtime for concurrent operations
- **System calls**: Direct interaction with `/proc` filesystem

## License

This project is open source. See the LICENSE file for details.
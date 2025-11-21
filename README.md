# Network Monitor

A real-time network connection monitoring tool that displays active connections with live I/O statistics.

## Features

- **Real-time monitoring**: Continuously monitors active network connections
- **I/O statistics**: Shows live upload/download rates for each connection
- **Process identification**: Displays the program and PID associated with each connection
- **Modern terminal UI**: Clean, color-coded table interface with activity indicators
- **Address resolution**: Simplifies common addresses (localhost, any, mDNS)
- **Connection filtering**: Filters out localhost connections for cleaner output

## Requirements

- Python 3.13+
- Linux system with `/proc` filesystem
- `ss` command (usually available with iproute2 package)

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd network-monitor
```

2. Install dependencies using uv:
```bash
uv sync
```

## Usage

Run the network monitor:
```bash
uv run python network-monitor.py
```

Or make it executable and run directly:
```bash
chmod +x network-monitor.py
./network-monitor.py
```

## Output

The monitor displays a real-time table with the following columns:

- **Protocol**: TCP/UDP protocol
- **State**: Connection state (ESTABLISHED, LISTEN, etc.)
- **Local Address**: Local endpoint (resolved to readable format)
- **Remote Address**: Remote endpoint (resolved to readable format)
- **Program(PID)**: Process name and PID
- **RX Rate**: Download rate (blue when active)
- **TX Rate**: Upload rate (red when active)

### Color Coding

- 🟢 **Green rows**: Bidirectional activity (both upload and download)
- 🔵 **Blue rows**: Download only activity
- 🔴 **Red rows**: Upload only activity
- ⚪ **White rows**: No recent activity

### Address Resolution

Common addresses are simplified for readability:
- `0.0.0.0:*` or `*:*` → `ANY`
- `127.0.0.1:*` or `[::1]:*` → `LOCALHOST`
- `224.0.0.251:*` → `MDNS`

## Controls

- **Ctrl+C**: Stop monitoring and exit

## How It Works

1. Uses `ss -tulnape` to get active connections with process information
2. Reads `/proc/[pid]/io` for real-time I/O statistics
3. Calculates rates by comparing I/O between updates
4. Updates display every second with current connection state

## License

This project is open source. See the LICENSE file for details.
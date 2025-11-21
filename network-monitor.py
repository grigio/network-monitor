#!/usr/bin/env python3
"""
Simple network connection monitor with real I/O statistics
"""

import subprocess
import time
import re
import socket
import os
from datetime import datetime

def get_connections():
    """Get active connections with program info"""
    try:
        result = subprocess.run(['ss', '-tulnape'], capture_output=True, text=True)
        lines = result.stdout.strip().split('\n')[1:]
        
        connections = []
        for line in lines:
            if not line.strip():
                continue
                
            parts = line.split()
            if len(parts) < 6:
                continue
                
            protocol = parts[0]
            state = parts[1]
            local_addr = parts[4]
            remote_addr = parts[5]
            
            program = 'N/A'
            pid = 'N/A'
            for part in parts:
                if part.startswith('users:(('):
                    pid_match = re.search(r'pid=(\d+)', part)
                    if pid_match:
                        pid = pid_match.group(1)
                    prog_match = re.search(r'"([^"]+)"', part)
                    if prog_match:
                        program = prog_match.group(1)
                    break
            
            # If no PID found, try to guess based on port and known processes
            if pid == 'N/A' and remote_addr.endswith(':22'):
                # SSH connection - try to find SSH processes
                try:
                    result2 = subprocess.run(['ps', 'aux'], capture_output=True, text=True)
                    for line in result2.stdout.split('\n'):
                        if 'ssh' in line and not line.startswith('root'):
                            parts2 = line.split()
                            if len(parts2) > 1:
                                possible_pid = parts2[1]
                                # Check if this PID has network activity
                                try:
                                    with open(f'/proc/{possible_pid}/io', 'r') as f:
                                        io_data = f.read()
                                        for io_line in io_data.split('\n'):
                                            if io_line.startswith('read_bytes:'):
                                                read_bytes = int(io_line.split()[1])
                                                if read_bytes > 0:  # Only if there's actual I/O
                                                    pid = possible_pid
                                                    program = 'ssh'
                                                    break
                                except:
                                    continue
                                if pid != 'N/A':
                                    break
                except:
                    pass
            
            connections.append({
                'protocol': protocol,
                'state': state,
                'local': local_addr,
                'remote': remote_addr,
                'program': program,
                'pid': pid
            })
        
        return connections
    except Exception as e:
        print(f"Error getting connections: {e}")
        return []

def get_process_io(pid):
    """Get process I/O statistics"""
    try:
        with open(f'/proc/{pid}/io', 'r') as f:
            io_data = f.read()
            rx_bytes = 0  # Received bytes (network input)
            tx_bytes = 0  # Transmitted bytes (network output)
            for line in io_data.split('\n'):
                if line.startswith('rchar:'):  # Characters read (includes network)
                    rx_bytes = int(line.split()[1])
                elif line.startswith('wchar:'):  # Characters written (includes network)
                    tx_bytes = int(line.split()[1])
            return rx_bytes, tx_bytes
    except:
        return 0, 0

def resolve_address(addr):
    """Resolve IP address to hostname or simplified format"""
    if addr == "0.0.0.0:*" or addr == "*:*" or addr == "[::]:*":
        return "ANY"
    elif addr.startswith("127.0.0.1:") or addr.startswith("[::1]:"):
        return "LOCALHOST"
    elif addr.startswith("224.0.0.251:"):
        return "MDNS"
    else:
        return addr

def format_bytes(bytes_val):
    """Format bytes to human readable format"""
    for unit in ['B', 'KB', 'MB', 'GB']:
        if bytes_val < 1024.0:
            return f"{bytes_val:.1f}{unit}"
        bytes_val /= 1024.0
    return f"{bytes_val:.1f}TB"

def print_table_header():
    """Print modern table header with styling"""
    bold = "\033[1m"
    cyan = "\033[96m"
    reset = "\033[0m"
    
    # Column widths: Protocol(8), State(12), Local(20), Remote(25), Program(20), RX(12), TX(12)
    # Total width: 109 + 6 borders = 115
    
    # Top border
    print(f"┌{'─' * 8}┬{'─' * 12}┬{'─' * 20}┬{'─' * 25}┬{'─' * 20}┬{'─' * 12}┬{'─' * 12}┐")
    
    # Header row - left aligned
    header = f"│{bold}{cyan}{'Protocol':<8}{reset}│{bold}{cyan}{'State':<12}{reset}│{bold}{cyan}{'Local Address':<20}{reset}│{bold}{cyan}{'Remote Address':<25}{reset}│{bold}{cyan}{'Program(PID)':<20}{reset}│{bold}{cyan}{'RX Rate':<12}{reset}│{bold}{cyan}{'TX Rate':<12}{reset}│"
    print(header)
    
    # Separator
    print(f"├{'─' * 8}┼{'─' * 12}┼{'─' * 20}┼{'─' * 25}┼{'─' * 20}┼{'─' * 12}┼{'─' * 12}┤")

def print_table_row(conn, rx_rate, tx_rate):
    """Print a single table row with modern styling"""
    reset = "\033[0m"
    
    prog_pid = "{}({})".format(conn['program'], conn['pid']) if conn['pid'] != "N/A" else conn['program']
    local_resolved = resolve_address(conn['local'])
    remote_resolved = resolve_address(conn['remote'])
    
    # Determine row color based on activity
    if rx_rate > 0 and tx_rate > 0:
        row_color = "\033[92m"  # Green for bidirectional
    elif rx_rate > 0:
        row_color = "\033[94m"  # Blue for download only (RX)
    elif tx_rate > 0:
        row_color = "\033[91m"  # Red for upload only (TX)
    else:
        row_color = ""
    
    # Truncate long fields to fit column width
    prog_pid = prog_pid[:19] if len(prog_pid) > 20 else prog_pid
    local_resolved = local_resolved[:19] if len(local_resolved) > 20 else local_resolved
    remote_resolved = remote_resolved[:24] if len(remote_resolved) > 25 else remote_resolved
    
    # Format rates with colors
    rx_text = format_bytes(rx_rate) + "/s"
    tx_text = format_bytes(tx_rate) + "/s"
    rx_formatted = f"\033[94m{rx_text:<11}{reset}" if rx_rate > 0 else f"{rx_text:<11}"  # Blue for RX (download)
    tx_formatted = f"\033[91m{tx_text:<11}{reset}" if tx_rate > 0 else f"{tx_text:<11}"  # Red for TX (upload)
    
    # Build row with borders - left aligned columns
    row = f"{row_color}│{conn['protocol']:<8}│{conn['state']:<12}│{local_resolved:<20}│{remote_resolved:<25}│{prog_pid:<20}│{rx_formatted}│{tx_formatted}│{reset}"
    print(row)

def print_table_footer():
    """Print table footer"""
    print(f"└{'─' * 8}┴{'─' * 12}┴{'─' * 20}┴{'─' * 25}┴{'─' * 20}┴{'─' * 12}┴{'─' * 12}┘")

def main():
    """Main monitoring loop"""
    prev_io = {}
    
    try:
        while True:
            # Clear screen and move cursor to top-left
            os.system('clear')
            print('\033[?25l', end='', flush=True)
            
            # Modern header with gradient effect
            print(f"\033[96m╔{'═' * 115}╗\033[0m")
            print(f"\033[96m║\033[1m{'🌐 Live Network Connections Monitor':^115}\033[0m\033[96m║\033[0m")
            print(f"\033[96m║{datetime.now().strftime('%Y-%m-%d %H:%M:%S'):^115}║\033[0m")
            print(f"\033[96m╚{'═' * 115}╝\033[0m")
            print()
            
            # Get connections
            connections = get_connections()
            
            # Display connections table
            if connections:
                print_table_header()
                
                active_connections = 0
                for conn in connections:
                    # Skip localhost connections
                    if resolve_address(conn['remote']) == "LOCALHOST":
                        continue
                    
                    # Calculate rates for this connection
                    rx_rate, tx_rate = 0, 0
                    if conn['pid'] != "N/A":
                        current_read, current_write = get_process_io(conn['pid'])
                        pid_key = conn['pid']
                        
                        if pid_key in prev_io:
                            rx_rate = current_read - prev_io[pid_key]['rx']
                            tx_rate = current_write - prev_io[pid_key]['tx']
                        
                        prev_io[pid_key] = {'rx': current_read, 'tx': current_write}
                    
                    print_table_row(conn, rx_rate, tx_rate)
                    if rx_rate > 0 or tx_rate > 0:
                        active_connections += 1
                
                print_table_footer()
                
                # Summary line
                print(f"\n\033[96m📊 Summary: {len(connections)} total connections, {active_connections} active\033[0m")
                print(f"\033[90mLegend: \033[92m● Bidirectional\033[0m  \033[94m↓ Download only\033[0m  \033[91m↑ Upload only\033[0m")
            else:
                print(f"\033[91m🔍 No active connections found\033[0m")
            
            # Wait before next update (1 second)
            time.sleep(1)
            
    except KeyboardInterrupt:
        # Show cursor again before exiting
        print('\033[?25h')
        print(f"\n\033[92m✓ Monitoring stopped by user\033[0m")

if __name__ == "__main__":
    main()

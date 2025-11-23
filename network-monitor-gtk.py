#!/usr/bin/env python3
"""
GTK UI version of network connection monitor with real I/O statistics
"""

import subprocess
import re
import socket
import os
import threading
from datetime import datetime
import gi

# Disable the problematic GTK setting before importing
os.environ['GTK_THEME'] = ''

gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Gio, GLib, Gdk

class NetworkMonitorWindow(Adw.ApplicationWindow):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.set_title("Network Monitor")
        self.set_default_size(1200, 800)
        
        # Set up Adwaita style manager for proper theming
        style_manager = Adw.StyleManager.get_default()
        style_manager.set_color_scheme(Adw.ColorScheme.DEFAULT)
        
        self.prev_io = {}
        self.update_interval = 3000  # 3 seconds
        self.sort_column = 0  # Default sort column
        self.sort_ascending = True  # Default sort direction
        self.header_buttons = []  # Initialize header buttons list
        self.resolve_hosts = True  # Host resolution toggle state
        self.resolution_cache = {}  # Cache for resolved hostnames
        self.resolution_pending = set()  # Track pending resolutions
        self.resolution_semaphore = threading.Semaphore(5)  # Limit concurrent resolutions
        
        self.setup_ui()
        self.start_monitoring()
    
    def setup_ui(self):
        # Create main box
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        self.set_content(main_box)
        
        # Header bar
        header_bar = Adw.HeaderBar()
        main_box.append(header_bar)
        
        # Create menu button
        menu_button = Gtk.MenuButton()
        menu_model = self.create_menu_model()
        menu_button.set_menu_model(menu_model)
        header_bar.pack_end(menu_button)
        
        # Initialize header labels with sort indicator
        self.update_header_labels()
        
        # Main content area
        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        content_box.set_margin_top(6)
        content_box.set_margin_bottom(6)
        content_box.set_margin_start(6)
        content_box.set_margin_end(6)
        main_box.append(content_box)
        
        # Create scrolled window for tree view
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.AUTOMATIC)
        scrolled.set_vexpand(True)
        content_box.append(scrolled)
        
        # Create tree view
        self.create_tree_view()
        scrolled.set_child(self.grid)
        
        # Status bar
        self.status_bar = Adw.StatusPage()
        self.status_bar.set_title("Network Monitor")
        self.status_bar.set_description("Monitoring network connections...")
        content_box.append(self.status_bar)
        
        # Bottom control panel
        control_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        control_box.set_margin_top(10)
        control_box.set_margin_bottom(10)
        control_box.set_margin_start(10)
        control_box.set_margin_end(10)
        control_box.set_halign(Gtk.Align.CENTER)
        content_box.append(control_box)
        
        # Host resolution toggle
        self.resolve_toggle = Gtk.ToggleButton(label="Resolve Hostnames")
        self.resolve_toggle.set_active(self.resolve_hosts)
        self.resolve_toggle.connect("toggled", self.on_resolve_toggled)
        control_box.append(self.resolve_toggle)
        
        # Update status
        self.update_status()
    
    def create_menu_model(self):
        menu = Gio.Menu()
        
        menu.append("Refresh", "app.refresh")
        menu.append("About", "app.about")
        
        return menu
    
    def create_tree_view(self):
        # Create a simple grid view for GTK4
        self.grid = Gtk.Grid()
        self.grid.set_column_spacing(10)
        self.grid.set_row_spacing(5)
        
        # Create clickable header buttons
        headers = ["Process(ID)", "Protocol", "Source", "Destination", "Status", "TX", "RX", "Path"]
        for i, header in enumerate(headers):
            button = Gtk.Button(label=header)
            button.add_css_class("flat")
            button.add_css_class("heading")
            button.connect("clicked", self.on_header_clicked, i)
            self.grid.attach(button, i, 0, 1, 1)
            self.header_buttons.append(button)
        
        # Store for connection data
        self.connections_data = []
        self.row_widgets = []
    
    def get_connections(self):
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
    
    def get_process_io(self, pid):
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
    
    def get_process_path(self, pid):
        """Get process full path with arguments"""
        try:
            # Get command line from /proc/[pid]/cmdline
            with open(f'/proc/{pid}/cmdline', 'r') as f:
                cmdline = f.read().strip()
                if cmdline:
                    # Replace null bytes with spaces for display
                    return cmdline.replace('\0', ' ')
                else:
                    # If cmdline is empty, it might be a kernel thread
                    return f"[{pid}]"
        except:
            return "N/A"
    
    def on_header_clicked(self, button, column_index):
        """Handle header click for sorting"""
        if self.sort_column == column_index:
            # Toggle sort direction if same column
            self.sort_ascending = not self.sort_ascending
        else:
            # New column, set ascending
            self.sort_column = column_index
            self.sort_ascending = True
        
        # Update header button labels to show sort direction
        self.update_header_labels()
        
        # Refresh the display with new sort
        self.update_connections()
    
    def update_header_labels(self):
        """Update header labels to show sort direction"""
        headers = ["Process(ID)", "Protocol", "Source", "Destination", "Status", "TX", "RX", "Path"]
        for i, (button, header) in enumerate(zip(self.header_buttons, headers)):
            if i == self.sort_column:
                arrow = " ▲" if self.sort_ascending else " ▼"
                button.set_label(header + arrow)
            else:
                button.set_label(header)
    
    def sort_connections(self, connections):
        """Sort connections based on current sort column and direction"""
        if not connections:
            return connections
        
        # Define sort key functions for each column
        def get_sort_key(conn):
            # For numeric columns (TX, RX), use the raw rate values
            if self.sort_column == 5:  # TX column
                return conn.get('tx_rate', 0)
            elif self.sort_column == 6:  # RX column
                return conn.get('rx_rate', 0)
            
            # For other columns, get the formatted display value
            row_data = self.get_row_data(conn)
            if self.sort_column >= len(row_data):
                return ""
            
            value = row_data[self.sort_column]
            # For string columns, return lowercase for case-insensitive sorting
            return str(value).lower()
        
        # Sort the connections
        return sorted(connections, key=get_sort_key, reverse=not self.sort_ascending)
    
    def get_row_data(self, conn):
        """Get formatted row data for a connection"""
        # Use pre-calculated rates from the connection
        rx_rate = conn.get('rx_rate', 0)
        tx_rate = conn.get('tx_rate', 0)
        
        # Format display values
        prog_pid = "{}({})".format(conn['program'], conn['pid']) if conn['pid'] != "N/A" else conn['program']
        local_resolved = self.resolve_address(conn['local'])
        remote_resolved = self.resolve_address(conn['remote'])
        
        # Truncate long fields
        prog_pid = prog_pid[:30] if len(prog_pid) > 30 else prog_pid
        local_resolved = local_resolved[:30] if len(local_resolved) > 30 else local_resolved
        remote_resolved = remote_resolved[:30] if len(remote_resolved) > 30 else remote_resolved
        
        # Get process path
        process_path = self.get_process_path(conn['pid']) if conn['pid'] != "N/A" else "N/A"
        
        return [
            prog_pid,  # Process(ID)
            conn['protocol'],  # Protocol
            local_resolved,  # Source
            remote_resolved,  # Destination
            conn['state'],  # Status
            self.format_bytes(tx_rate),  # TX
            self.format_bytes(rx_rate),  # RX
            process_path  # Path
        ]
    
    def resolve_address(self, addr):
        """Resolve IP address to hostname or simplified format"""
        if addr == "0.0.0.0:*" or addr == "*:*" or addr == "[::]:*":
            return "ANY"
        elif addr.startswith("127.0.0.1:") or addr.startswith("[::1]:"):
            return "LOCALHOST"
        elif addr.startswith("224.0.0.251:"):
            return "MDNS"
        else:
            if not self.resolve_hosts:
                return addr
            
            # Check cache first
            if addr in self.resolution_cache:
                return self.resolution_cache[addr]
            
            # Extract IP address (remove port)
            if ':' in addr:
                ip_part = addr.rsplit(':', 1)[0]
                # Handle IPv6 addresses
                if ip_part.startswith('[') and ip_part.endswith(']'):
                    ip_part = ip_part[1:-1]
                
                # Start async resolution if not already pending
                if ip_part not in self.resolution_pending:
                    self.resolution_pending.add(ip_part)
                    thread = threading.Thread(target=self.async_resolve_host, args=(ip_part, addr))
                    thread.daemon = True
                    thread.start()
                
                # Return original address for now
                return addr
            else:
                return addr
    
    def async_resolve_host(self, ip_part, full_addr):
        """Asynchronously resolve hostname and update cache"""
        with self.resolution_semaphore:  # Limit concurrent DNS lookups
            try:
                hostname = socket.gethostbyaddr(ip_part)[0]
                resolved = f"{hostname}:{full_addr.rsplit(':', 1)[1]}"
            except:
                resolved = full_addr
            
            # Update cache and schedule UI refresh
            GLib.idle_add(self.update_resolution_cache, full_addr, resolved)
    
    def update_resolution_cache(self, addr, resolved):
        """Update resolution cache and trigger UI refresh (called from main thread)"""
        self.resolution_cache[addr] = resolved
        if addr in self.resolution_pending:
            self.resolution_pending.discard(addr.split(':')[0])  # Remove IP part
        
        # Trigger a refresh to show resolved names
        self.update_connections()
    
    def on_resolve_toggled(self, button):
        """Handle host resolution toggle"""
        self.resolve_hosts = button.get_active()
        
        if not self.resolve_hosts:
            # Clear cache when disabling resolution
            self.resolution_cache.clear()
            self.resolution_pending.clear()
        
        # Schedule refresh in main loop to avoid blocking
        GLib.idle_add(self.delayed_refresh)
    
    def delayed_refresh(self):
        """Delayed refresh to avoid blocking UI"""
        self.prev_io.clear()
        self.update_connections()
        return False  # Don't repeat this function
    
    def format_bytes(self, bytes_val):
        """Format bytes to human readable format"""
        for unit in ['B', 'KB', 'MB', 'GB']:
            if bytes_val < 1024.0:
                return f"{bytes_val:.1f}{unit}/s"
            bytes_val /= 1024.0
        return f"{bytes_val:.1f}TB/s"
    
    def get_row_color(self, rx_rate, tx_rate):
        """Determine row color based on activity"""
        if rx_rate > 0 and tx_rate > 0:
            return "#00AA00"  # Green for bidirectional
        elif rx_rate > 0:
            return "#0066CC"  # Blue for download only (RX)
        elif tx_rate > 0:
            return "#CC0000"  # Red for upload only (TX)
        else:
            return None
    
    def update_connections(self):
        """Update the connection list"""
        # Clear current grid rows (except header)
        for widget in self.row_widgets:
            self.grid.remove(widget)
        self.row_widgets.clear()
        
        # Get connections
        connections = self.get_connections()
        
        # Update I/O data for rate calculations
        current_io = {}
        for conn in connections:
            if conn['pid'] != "N/A":
                current_read, current_write = self.get_process_io(conn['pid'])
                pid_key = conn['pid']
                current_io[pid_key] = {'rx': current_read, 'tx': current_write}
                
                # Calculate rates based on previous I/O data
                if pid_key in self.prev_io:
                    # Ensure we don't get negative rates (which can happen on counter wrap)
                    rx_diff = max(0, current_read - self.prev_io[pid_key]['rx'])
                    tx_diff = max(0, current_write - self.prev_io[pid_key]['tx'])
                    conn['rx_rate'] = rx_diff
                    conn['tx_rate'] = tx_diff
                else:
                    conn['rx_rate'] = 0
                    conn['tx_rate'] = 0
            else:
                conn['rx_rate'] = 0
                conn['tx_rate'] = 0
        
        # Update previous I/O data for next iteration
        self.prev_io = current_io
        
        # Filter out localhost connections
        filtered_connections = [
            conn for conn in connections 
            if self.resolve_address(conn['remote']) != "LOCALHOST"
        ]
        
        # Sort connections
        sorted_connections = self.sort_connections(filtered_connections)
        
        active_connections = 0
        row = 1  # Start from row 1 (row 0 is headers)
        
        for conn in sorted_connections:
            # Get formatted row data
            row_data = self.get_row_data(conn)
            
            for col, text in enumerate(row_data):
                label = Gtk.Label(label=text)
                label.set_ellipsize(3)  # Pango.EllipsizeMode.END
                
                # Set alignment and color for rate columns
                if col == 5:  # TX Rate
                    label.set_halign(Gtk.Align.END)
                    label.add_css_class("error")
                elif col == 6:  # RX Rate
                    label.set_halign(Gtk.Align.END)
                    label.add_css_class("accent")
                else:
                    label.set_halign(Gtk.Align.START)
                    
                self.grid.attach(label, col, row, 1, 1)
                self.row_widgets.append(label)
            
            if conn['rx_rate'] > 0 or conn['tx_rate'] > 0:
                active_connections += 1
            
            row += 1
        
        # Update status
        self.update_status(len(connections), active_connections)
        
        # Schedule next update
        GLib.timeout_add(self.update_interval, self.update_connections)
    
    def update_status(self, total=0, active=0):
        """Update status bar"""
        if total > 0:
            self.status_bar.set_description(f"{total} total connections, {active} active")
        else:
            self.status_bar.set_description("Monitoring network connections...")
    

    
    def start_monitoring(self):
        """Start the monitoring loop"""
        self.update_connections()

class NetworkMonitorApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id='com.example.NetworkMonitor')
        self.window = None
        
        # Set up style manager at application level
        style_manager = Adw.StyleManager.get_default()
        style_manager.set_color_scheme(Adw.ColorScheme.DEFAULT)
        
        # Add actions
        self.create_actions()
    
    def create_actions(self):
        # Refresh action
        refresh_action = Gio.SimpleAction.new('refresh', None)
        refresh_action.connect('activate', self.on_refresh)
        self.add_action(refresh_action)
        
        # About action
        about_action = Gio.SimpleAction.new('about', None)
        about_action.connect('activate', self.on_about)
        self.add_action(about_action)
    

    
    def on_refresh(self, action, parameter):
        """Force refresh"""
        if self.window:
            self.window.prev_io.clear()
            self.window.update_connections()
    
    def on_about(self, action, parameter):
        """Show about dialog"""
        about = Adw.AboutWindow()
        about.set_application_name("Network Monitor")
        about.set_version("1.0")
        about.set_developer_name("Network Monitor")
        about.set_license_type(Gtk.License.MIT_X11)
        about.set_comments("GTK UI version of network connection monitor with real I/O statistics")
        about.set_transient_for(self.window)
        about.present()
    
    def do_activate(self):
        if not self.window:
            self.window = NetworkMonitorWindow(application=self)
        self.window.present()

def main():
    app = NetworkMonitorApp()
    app.run()

if __name__ == "__main__":
    main()
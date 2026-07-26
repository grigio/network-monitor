# Migration: /proc-net to eBPF-only backend

## Date
2026-07-26

## Goal
Remove the legacy `/proc/net` polling backend and make eBPF the sole connection monitoring backend.

## Changes

### Removed files
- `src/services/network.rs` — `/proc/net` parsing engine (325 lines)
- `src/services/process_cache.rs` — inode-to-PID mapping cache (193 lines)
- `src/services/network_tests.rs` — NetworkService tests (39 lines)
- `src/services/tests.rs` — service integration tests (132 lines)
- `src/utils/recovery.rs` — circuit breaker / error recovery (460 lines)

### Modified files

| File | Change |
|------|--------|
| `Cargo.toml` | Removed features section; `aya` and `aya-log` are now hard dependencies (not optional) |
| `build.rs` | eBPF build failure is now a hard error; removed `#[cfg(feature = "ebpf")]` gates |
| `src/services/mod.rs` | Only exports `connection_monitor`, `ebpf_monitor`, `resolver` |
| `src/services/connection_monitor.rs` | Removed `ProcMonitor` struct and fallback; `detect_best_monitor()` always creates `EbpfMonitor`, exits on failure |
| `src/services/ebpf_monitor.rs` | Removed `ProcessCache` field; removed rehydration fallback to `NetworkService::new()`; added `kprobe_tcp_connect6` program load |
| `src/error.rs` | Removed `ProcIo`, `InvalidAddress`, `ProcessNotFound`, `HexParseError`, `InvalidPid` variants; added `Io(#[from] std::io::Error)` for TUI I/O |
| `src/error_tests.rs` | Removed tests for deleted variants (MutexPoison + ParseError + Result tests kept) |
| `src/models/connection.rs` | Removed `ProcessInfo` struct; removed unused `Connection::new()` |
| `src/utils/parsing.rs` | Removed `/proc/net`-specific functions: `parse_ipv4_hex`, `parse_ipv6_hex`, `parse_tcp_state`, `split_socket_addr`, `validate_pid`, `parse_inode`, `normalize_address`, `parse_port` |
| `src/utils/mod.rs` | Removed `recovery` module; removed unused re-exports |
| `network-monitor-ebpf/src/main.rs` | Added `kprobe_tcp_connect6` function (shares logic with IPv4, attaches to `tcp_v6_connect`) |
| `README.md` | Updated to reflect eBPF-only mode; removed `--features ebpf` instructions |
| `AGENTS.md` | Updated architecture documentation, commands, and pitfalls |
| `flake.nix` | Updated dev shell banner |

## Build issues encountered

1. **bpf-linker LLVM compatibility** — Nix-provided bpf-linker (LLVM 21) was incompatible with nightly's LLVM 22. Fixed by installing bpf-linker via `cargo install` under the nightly toolchain.

2. **build.rs: nightly cargo finds wrong rustc** — `RUSTC_WORKSPACE_WRAPPER` (set by clippy) caused the nightly cargo to use the Nix clippy-driver for sysroot detection in `-Z build-std=core`. Fixed by adding `.env_remove("RUSTC_WORKSPACE_WRAPPER")` to the nightly cargo Command.

3. **Missing `kprobe_tcp_connect6`** — The eBPF kernel code defined `kprobe_tcp_connect`, `kprobe_tcp_close`, and `kretprobe_inet_csk_accept` but `ebpf_monitor.rs` tried to load a program named `kprobe_tcp_connect6` that didn't exist in the BPF ELF. Fixed by adding the missing kprobe.

4. **Capability issues on host** — File capabilities (`setcap cap_bpf,cap_net_admin,cap_perfmon+ep`) don't elevate `CapEff` on the host system due to unknown environment/policy reasons. Workaround: run via `sudo` with `-E` flag.

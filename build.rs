use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("network-monitor-ebpf");

    if let Err(e) = try_build_ebpf(&dst) {
        eprintln!("Warning: {e}");
        eprintln!("Warning: eBPF programs not built; will skip eBPF tracing at runtime");
        std::fs::write(&dst, [0u8; 4]).ok();
    }
}

fn run_nightly_cargo(args: &[&str], ebpf_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut nightly_cargo = find_nightly_cargo()?;

    let output = nightly_cargo
        .args(args)
        .current_dir(ebpf_dir)
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .output()
        .map_err(|e| format!("Failed to run nightly cargo: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Nightly cargo build failed:\n{stderr}").into());
    }
    Ok(())
}

fn find_nightly_cargo() -> Result<Command, Box<dyn std::error::Error>> {
    // 1) Try `cargo +nightly` (rustup proxy)
    let probe = Command::new("cargo")
        .args(["+nightly", "--version"])
        .output();
    if let Ok(out) = probe {
        if out.status.success() {
            let mut cmd = Command::new("cargo");
            cmd.arg("+nightly");
            return Ok(cmd);
        }
    }

    // 2) Try `rustup run nightly cargo`
    let probe = Command::new("rustup")
        .args(["run", "nightly", "cargo", "--version"])
        .output();
    if let Ok(out) = probe {
        if out.status.success() {
            let mut cmd = Command::new("rustup");
            cmd.args(["run", "nightly", "cargo"]);
            return Ok(cmd);
        }
    }

    // 3) Scan rustup toolchains directory
    if let Ok(home) = std::env::var("RUSTUP_HOME")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.rustup")))
    {
        let tc = Path::new(&home).join("toolchains");
        if let Ok(entries) = std::fs::read_dir(&tc) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.contains("nightly") {
                    let cargo = entry.path().join("bin").join("cargo");
                    if cargo.exists() {
                        let probe = Command::new(&cargo).arg("--version").output();
                        if let Ok(out) = probe {
                            if out.status.success() {
                                return Ok(Command::new(cargo));
                            }
                        }
                    }
                }
            }
        }
    }

    Err("Cannot locate nightly Rust toolchain. Install it with: rustup toolchain install nightly && rustup target add bpfel-unknown-none --toolchain nightly && cargo install bpf-linker".into())
}

fn try_build_ebpf(dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let ebpf_dir = Path::new("network-monitor-ebpf");
    let target = "bpfel-unknown-none";

    run_nightly_cargo(
        &[
            "build",
            "--release",
            "--target",
            target,
            "-Z",
            "build-std=core",
        ],
        ebpf_dir,
    )?;

    let binary = ebpf_dir
        .join("target")
        .join(target)
        .join("release")
        .join("network-monitor-ebpf");

    if binary.exists() {
        std::fs::copy(&binary, dst).map_err(|e| format!("Failed to copy {binary:?}: {e}"))?;
        eprintln!("Info: eBPF programs built successfully");
        return Ok(());
    }

    let deps_dir = ebpf_dir
        .join("target")
        .join(target)
        .join("release")
        .join("deps");
    for entry in std::fs::read_dir(&deps_dir)
        .map_err(|e| format!("Cannot read deps dir {deps_dir:?}: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Cannot read entry: {e}"))?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "o" || ext == "rlib" {
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if name.starts_with("network_monitor_ebpf") {
                    std::fs::copy(&path, dst)
                        .map_err(|e| format!("Failed to copy {path:?} to {dst:?}: {e}"))?;
                    eprintln!("Info: eBPF programs built successfully");
                    return Ok(());
                }
            }
        }
    }

    Err("No eBPF artifact found in build output".into())
}

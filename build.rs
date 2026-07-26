use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("network-monitor-ebpf");

    try_build_ebpf(&dst).expect("Failed to build eBPF programs (required)");
}

fn try_build_ebpf(dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let nightly_root = Path::new(
        "/home/grigio/Code/sandbox-bwrap-nix/sandbox-home/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu",
    );
    let nightly_lib = nightly_root.join("lib");
    let cargo_bin = Path::new("/home/grigio/Code/sandbox-bwrap-nix/sandbox-home/.cargo/bin");

    let nightly_bin = nightly_root.join("bin");

    let new_path = format!(
        "{}:{}:{}",
        nightly_bin.display(),
        cargo_bin.display(),
        std::env::var("PATH").unwrap_or_default(),
    );
    let new_ld_path = format!(
        "{}:{}",
        nightly_lib.display(),
        std::env::var("LD_LIBRARY_PATH").unwrap_or_default(),
    );
    std::env::set_var("PATH", &new_path);
    std::env::set_var("LD_LIBRARY_PATH", &new_ld_path);

    let nightly_cargo = nightly_bin.join("cargo");
    let ebpf_dir = Path::new("network-monitor-ebpf");
    let target = "bpfel-unknown-none";

    let mut cmd = Command::new(&nightly_cargo);
    cmd.args([
        "build",
        "--release",
        "--target",
        target,
        "-Z",
        "build-std=core",
    ])
    .current_dir(ebpf_dir)
    .env("RUSTC", nightly_bin.join("rustc"))
    .env("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "gcc")
    .env_remove("RUSTC_WORKSPACE_WRAPPER");

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run nightly cargo: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Nightly cargo build failed:\n{stderr}").into());
    }

    let binary = ebpf_dir
        .join("target")
        .join(target)
        .join("release")
        .join("network-monitor-ebpf");
    let found = if binary.exists() {
        std::fs::copy(&binary, dst).map_err(|e| format!("Failed to copy {binary:?}: {e}"))?;
        true
    } else {
        false
    };

    if !found {
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
                            .map_err(|e| format!("Failed to copy {:?} to {:?}: {e}", path, dst))?;
                        break;
                    }
                }
            }
        }
    }

    if !found {
        return Err("No eBPF artifact found in deps dir".into());
    }

    eprintln!("Info: eBPF programs built successfully");
    Ok(())
}

use std::env;
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::process::CommandExt;
use std::process::Command;

fn main() {
    let home = env::var("HOME").expect("HOME environment variable not set");
    let xdg_runtime_dir =
        env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR environment variable not set");
    let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_string());
    let wayland_socket = format!("{}/{}", xdg_runtime_dir, wayland_display);

    // Ensure history and config directories exist on host
    let local_share = format!("{}/.local/share/carnet", home);
    let config_dir = format!("{}/.config/carnet", home);
    let _ = fs::create_dir_all(&local_share);
    let _ = fs::create_dir_all(&config_dir);

    match fs::metadata(&wayland_socket) {
        Ok(meta) => {
            if !meta.file_type().is_socket() {
                eprintln!(
                    "Error: Wayland socket at {} is not a socket",
                    wayland_socket
                );
                std::process::exit(1);
            }
        }
        Err(_) => {
            eprintln!("Error: Wayland socket not found at {}", wayland_socket);
            std::process::exit(1);
        }
    }

    // Process additional read-only paths from environment variable
    let mut extra_args = Vec::new();
    if let Ok(paths) = env::var("CARNET_EXTRA_PATHS") {
        for p in paths.split('\n') {
            if !p.is_empty() {
                extra_args.push("--ro-bind".to_string());
                extra_args.push(p.to_string());
                extra_args.push(p.to_string());
            }
        }
    }

    let term = env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string());
    let clipboard_state = env::var("CLIPBOARD_STATE").unwrap_or_default();

    let mut bwrap = Command::new("bwrap");
    bwrap
        .arg("--unshare-all")
        .arg("--new-session")
        .arg("--clearenv")
        .args(&["--setenv", "WAYLAND_DISPLAY", &wayland_display])
        .args(&["--setenv", "XDG_RUNTIME_DIR", &xdg_runtime_dir])
        .args(&["--setenv", "TERM", &term])
        .args(&["--setenv", "HOME", &home])
        .args(&["--setenv", "PATH", "/usr/bin:/bin"])
        .args(&["--setenv", "CLIPBOARD_STATE", &clipboard_state])
        .args(&["--setenv", "CARNET_SANDBOXED", "1"])
        .arg("--chdir")
        .arg(&home)
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--tmpfs")
        .arg("/tmp")
        .args(&["--ro-bind", "/usr/lib", "/usr/lib"])
        .args(&["--ro-bind", "/usr/lib64", "/usr/lib64"])
        .args(&["--ro-bind", "/usr/libexec", "/usr/libexec"])
        .args(&["--ro-bind", "/usr/share", "/usr/share"])
        .args(&["--ro-bind", "/lib", "/lib"])
        .args(&["--ro-bind", "/lib64", "/lib64"])
        .args(&["--ro-bind", "/usr/bin", "/usr/bin"])
        .args(&["--ro-bind", "/bin", "/bin"])
        .args(&["--ro-bind", &wayland_socket, &wayland_socket])
        .args(&["--bind", &local_share, &local_share])
        .args(&["--bind", &config_dir, &config_dir]);

    for arg in extra_args {
        bwrap.arg(arg);
    }

    // Bind the binary to /carnet
    let self_exe = env::current_exe().expect("Failed to get current executable path");
    let carnet_bin = self_exe
        .parent()
        .expect("Failed to get bin dir")
        .join("carnet");

    if !carnet_bin.exists() {
        eprintln!("Error: carnet binary not found at {:?}", carnet_bin);
        std::process::exit(1);
    }

    bwrap.args(&["--bind", carnet_bin.to_str().unwrap(), "/carnet"]);

    bwrap.arg("--").arg("/carnet");

    // Pass all arguments from this process to carnet
    let args: Vec<String> = env::args().skip(1).collect();
    bwrap.args(args);

    let err = bwrap.exec();
    eprintln!("Error: failed to exec bwrap: {}", err);
    std::process::exit(1);
}

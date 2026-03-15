use std::env;
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::process::Command;

pub fn create_base_bwrap_command() -> Command {
    let home = env::var("HOME").expect("HOME environment variable not set");
    let xdg_runtime_dir =
        env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR environment variable not set");
    let wayland_display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_string());
    let wayland_socket = format!("{}/{}", xdg_runtime_dir, wayland_display);

    match fs::metadata(&wayland_socket) {
        Ok(meta) => {
            if !meta.file_type().is_socket() {
                eprintln!(
                    "Warning: Wayland socket at {} is not a socket",
                    wayland_socket
                );
            }
        }
        Err(_) => {
            eprintln!("Warning: Wayland socket not found at {}", wayland_socket);
        }
    }

    let term = env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string());

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
        .args(&["--setenv", "CARNET_SANDBOXED", "1"])
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
        .args(&["--ro-bind", &wayland_socket, &wayland_socket]);

    bwrap
}

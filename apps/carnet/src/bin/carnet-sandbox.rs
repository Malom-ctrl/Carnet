use carnet::sandbox::create_base_bwrap_command;
use std::env;
use std::fs;
use std::os::unix::process::CommandExt;

fn main() {
    let home = env::var("HOME").expect("HOME environment variable not set");

    // Ensure history and config directories exist on host
    let local_share = format!("{}/.local/share/carnet", home);
    let config_dir = format!("{}/.config/carnet", home);
    let _ = fs::create_dir_all(&local_share);
    let _ = fs::create_dir_all(&config_dir);

    // Get base command with standard environment and mounts
    let mut bwrap = create_base_bwrap_command();

    // Add specific bindings for the main application
    bwrap.args(&["--bind", &local_share, &local_share]);
    bwrap.args(&["--bind", &config_dir, &config_dir]);

    // Process additional read-only paths from environment variable
    if let Ok(paths) = env::var("CARNET_EXTRA_PATHS") {
        for p in paths.split('\n') {
            if !p.is_empty() {
                bwrap.arg("--ro-bind");
                bwrap.arg(p);
                bwrap.arg(p);
            }
        }
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

use carnet::clipboard::get_image_paths_from_uri_list;
use carnet::config::Config;
use std::env;
use std::process::{Command, Stdio};

fn main() {
    let config = Config::load();

    // Assume carnet-sandbox is in the same directory as this executable
    let self_exe = env::current_exe().expect("Failed to get current executable path");
    let sandbox_bin = self_exe
        .parent()
        .expect("Failed to get bin dir")
        .join("carnet-sandbox");

    let mut cmd = Command::new(sandbox_bin);
    cmd.arg("store");

    if config.auto_convert_image_uri {
        let paths = get_image_paths_from_uri_list();
        if !paths.is_empty() {
            cmd.arg("--convert");
            let new_paths = paths.join("\n");
            let final_paths = if let Ok(existing) = env::var("CARNET_EXTRA_PATHS") {
                if existing.is_empty() {
                    new_paths
                } else {
                    format!("{}\n{}", existing, new_paths)
                }
            } else {
                new_paths
            };
            cmd.env("CARNET_EXTRA_PATHS", final_paths);
        }
    }

    // Redirect streams to null to ensure the caller (wl-paste)
    // doesn't wait for this process's descendants to close pipes.
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());

    // Spawn and exit immediately so carnet-preprocess returns to wl-paste
    if let Err(e) = cmd.spawn() {
        eprintln!("Error: failed to spawn carnet-sandbox: {}", e);
        std::process::exit(1);
    }
}

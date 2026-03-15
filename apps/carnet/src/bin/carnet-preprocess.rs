use carnet::clipboard;
use carnet::config::Config;
use carnet::sandbox::create_base_bwrap_command;
use std::env;
use std::process::{Command, Stdio};

fn main() {
    let config = Config::load();

    // Assume carnet-sandbox is in the same directory as this executable
    let self_exe = env::current_exe().expect("Failed to get current executable path");
    let bin_dir = self_exe.parent().expect("Failed to get bin dir");
    let sandbox_bin = bin_dir.join("carnet-sandbox");

    let mut cmd = Command::new(sandbox_bin);
    cmd.arg("store");

    if config.auto_convert_image_uri {
        // Run dedicated helper inside sandbox to get URIs without exposing raw clipboard to host
        let fetch_uris_bin = bin_dir.join("carnet-clipboard-fetch-uris");
        if fetch_uris_bin.exists() {
            let mut bwrap = create_base_bwrap_command();

            // Bind helper executable to a known path in sandbox
            bwrap.arg("--bind");
            bwrap.arg(&fetch_uris_bin);
            bwrap.arg("/carnet-clipboard-fetch-uris");

            // Command to run inside sandbox
            bwrap.arg("--");
            bwrap.arg("/carnet-clipboard-fetch-uris");

            // Capture output
            let output = bwrap.output().ok();

            if let Some(output) = output {
                if output.status.success() {
                    let content = String::from_utf8_lossy(&output.stdout);
                    let paths = clipboard::parse_uri_list(&content);
                    // Filter paths on host (checks file existence and magic numbers)
                    let valid_paths = clipboard::filter_image_paths(&paths);

                    if !valid_paths.is_empty() {
                        cmd.arg("--convert");
                        let new_paths = valid_paths.join("\n");
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
            }
        } else {
            eprintln!(
                "Warning: carnet-clipboard-fetch-uris not found at {:?}",
                fetch_uris_bin
            );
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

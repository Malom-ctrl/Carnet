use carnet::sandbox::create_base_bwrap_command;
use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn main() {
    watch_mode();
}

fn watch_mode() {
    let self_exe = env::current_exe().expect("Failed to get current executable path");
    let preprocess_bin = self_exe
        .parent()
        .expect("Failed to get bin dir")
        .join("carnet-preprocess");

    let mut cmd = create_base_bwrap_command();
    cmd.arg("--chdir").arg("/");

    cmd.stdout(Stdio::piped());

    cmd.arg("--");
    cmd.arg("wl-paste");
    cmd.arg("--watch");
    cmd.arg("echo");
    cmd.arg("change");

    let mut child = cmd
        .spawn()
        .expect("Failed to spawn sandboxed wl-paste --watch");

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(l) = line {
                if l.trim() == "change" {
                    let _ = Command::new(&preprocess_bin)
                        .spawn()
                        .and_then(|mut c| c.wait());
                }
            }
        }
    }

    // If the loop ends (pipe closed), wait for child
    let _ = child.wait();
}

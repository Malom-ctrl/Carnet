use std::env;
use std::process::Command;

fn main() {
    watch_mode();
}

fn watch_mode() {
    let self_exe = env::current_exe().expect("Failed to get current executable path");
    let preprocess_bin = self_exe.parent().expect("Failed to get bin dir").join("carnet-preprocess");

    let mut child = Command::new("wl-paste")
        .arg("--watch")
        .arg(preprocess_bin)
        .spawn()
        .expect("Failed to spawn wl-paste --watch");

    // Wait for the child to ensure we don't exit and kill the watcher
    let _ = child.wait();
}

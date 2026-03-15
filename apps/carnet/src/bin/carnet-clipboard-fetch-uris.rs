use carnet::clipboard;

fn main() {
    if std::env::var("CARNET_SANDBOXED").is_err() {
        eprintln!("Error: carnet-clipboard-fetch-uris must be run sandboxed.");
        std::process::exit(1);
    }
    if let Some(content) = clipboard::get_raw_uri_list_output() {
        print!("{}", content);
    }
}

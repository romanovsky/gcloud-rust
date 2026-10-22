fn main() {
    if let Err(e) = gcloud_rust::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

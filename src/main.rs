use std::io::IsTerminal;

use gcloud_rust::UsageError;

fn main() {
    if let Err(e) = gcloud_rust::run() {
        // A reader closing early (e.g. piping into `head`) must exit 0, not
        // print an error. This has to walk the *whole* chain, not just check
        // the top-level error: `.context()`/`.with_context()` wrap the
        // original `io::Error` inside an opaque anyhow link, so a top-level
        // `downcast_ref` alone would stop matching once context is attached.
        if is_broken_pipe(&e) {
            std::process::exit(0);
        }

        print_error_chain(&e);

        // Exit code convention: 0 success (above), 2 usage error, 1 anything
        // else (auth/network/API runtime failures).
        let code = if e.downcast_ref::<UsageError>().is_some() {
            2
        } else {
            1
        };
        std::process::exit(code);
    }
}

fn is_broken_pipe(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io_err| io_err.kind() == std::io::ErrorKind::BrokenPipe)
}

fn print_error_chain(e: &anyhow::Error) {
    if use_color() {
        eprintln!("\x1b[31merror:\x1b[0m {e}");
    } else {
        eprintln!("error: {e}");
    }
    for cause in e.chain().skip(1) {
        eprintln!("  caused by: {cause}");
    }
}

fn use_color() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

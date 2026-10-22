const USAGE_HEADER: &str = "\
gcloud-rust — a partial reimplementation of `gcloud`.

AVAILABLE COMMAND GROUPS:
    storage

";

pub(crate) enum Command {
    Storage(crate::storage::Command),
}

/// Top-level parse: consumes argv[0] and the command-group noun (currently
/// only `storage`), then hands the rest of the arguments to that group's own
/// parser. Adding a new group later means adding one more arm here plus its
/// own `<group>::cli` module -- this function stays a thin dispatcher.
pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> crate::Result<Command> {
    args.next(); // argv[0]

    match args.next().as_deref() {
        Some("storage") => Ok(Command::Storage(crate::storage::cli::parse(args)?)),
        Some("-h") | Some("--help") | None => print_usage_and_exit(),
        Some(other) => Err(crate::UsageError(format!(
            "unknown command `{other}`\n\n{USAGE_HEADER}"
        ))
        .into()),
    }
}

fn print_usage_and_exit() -> ! {
    print!("{USAGE_HEADER}");
    print!("{}", crate::storage::cli::USAGE);
    std::process::exit(0);
}

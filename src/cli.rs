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
///
/// `-V`/`--version` is checked here, before any subcommand dispatch, so it
/// never touches auth or the network -- same spirit as `-h`/`--help`.
pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> crate::Result<Command> {
    args.next(); // argv[0]

    match args.next().as_deref() {
        Some("storage") => Ok(Command::Storage(crate::storage::cli::parse(args)?)),
        Some("-V") | Some("--version") => print_version_and_exit(),
        Some("-h") | Some("--help") | None => print_usage_and_exit(),
        Some(other) => Err(crate::UsageError(format!(
            "unknown command `{other}`\n\n{USAGE_HEADER}"
        ))
        .into()),
    }
}

/// `gcloud-rust <version>`, where `<version>` is `Cargo.toml`'s `version` at
/// compile time -- the single source of truth, so this can never drift from
/// the crate version (and therefore from the release tag it ships under).
pub(crate) fn version_line() -> String {
    format!("gcloud-rust {}", env!("CARGO_PKG_VERSION"))
}

fn print_version_and_exit() -> ! {
    println!("{}", version_line());
    std::process::exit(0);
}

fn print_usage_and_exit() -> ! {
    print!("{USAGE_HEADER}");
    print!("{}", crate::storage::cli::USAGE);
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_line_is_name_plus_cargo_pkg_version() {
        let line = version_line();
        assert!(
            line.starts_with("gcloud-rust "),
            "expected a `gcloud-rust <version>` line, got {line:?}"
        );
        assert_eq!(&line["gcloud-rust ".len()..], env!("CARGO_PKG_VERSION"));
    }
}

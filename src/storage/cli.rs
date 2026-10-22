use crate::UsageError;

#[derive(Debug)]
pub(crate) enum Command {
    Ls { uri: String, recursive: bool },
    Cat { uri: String },
    Cp { src: String, dst: String, recursive: bool },
    Rm { uri: String, recursive: bool },
}

/// Short form: the top-level `storage --help` overview, and the tail of
/// "unknown subcommand" error messages. Per-subcommand detail lives in the
/// `*_HELP` constants below, shown by `storage <cmd> --help`.
pub(crate) const USAGE: &str = "\
storage — a partial reimplementation of `gcloud storage`.

USAGE:
    gcloud-rust storage ls [-r|-R|--recursive] <gs://bucket[/prefix]>
    gcloud-rust storage cat <gs://bucket/object>
    gcloud-rust storage cp [-r|-R|--recursive] <src> <dst>   (local<->gs://, or gs://<->gs://)
    gcloud-rust storage rm [-r|-R|--recursive] <gs://bucket/object>

Run `gcloud-rust storage <command> --help` for details on a specific command.

AUTH:
    Reads Application Default Credentials, set up via:
        gcloud auth application-default login
    Falls back to `gcloud auth print-access-token` (i.e. plain `gcloud auth
    login`) if ADC is absent.
";

const LS_HELP: &str = "\
storage ls — list objects and prefixes in Cloud Storage.

USAGE:
    gcloud-rust storage ls [-r|-R|--recursive] <gs://bucket[/prefix]>

FLAGS:
    -r, -R, --recursive   List every object under the prefix, grouped by
                          directory with header lines -- matches `gcloud
                          storage ls -r` output formatting exactly, including
                          the bucket/prefix-root header and the bare top-line
                          gcloud prints when a folder-placeholder object
                          exists at the queried path.
    -h, --help            Show this help.
";

const CAT_HELP: &str = "\
storage cat — print an object's contents to stdout.

USAGE:
    gcloud-rust storage cat <gs://bucket/object>

The whole object is buffered in memory before writing; fine for small/medium
objects, not recommended for multi-GB objects.

FLAGS:
    -h, --help   Show this help.
";

const CP_HELP: &str = "\
storage cp — copy objects to/from/within Cloud Storage.

USAGE:
    gcloud-rust storage cp [-r|-R|--recursive] <src> <dst>

    SRC/DST: a local filesystem path, or a gs://bucket/object URI. At least
    one side must be a gs:// URI (use the regular `cp` for local-to-local
    copies). gs://<->gs:// copies are performed server-side (no data is
    downloaded through this machine).

FLAGS:
    -r, -R, --recursive   Copy a whole directory/prefix. Destination nesting
                          follows the same existing-directory-vs-rename rule
                          as Unix `cp -r` / real `gcloud storage cp`.
    -h, --help            Show this help.
";

const RM_HELP: &str = "\
storage rm — delete objects, prefixes, or whole buckets.

USAGE:
    gcloud-rust storage rm [-r|-R|--recursive] <gs://bucket/object>

FLAGS:
    -r, -R, --recursive   Delete every object under the prefix. If the URI is
                          bucket-only (no object/prefix), also deletes the
                          now-empty bucket itself.
    -h, --help            Show this help.
";

/// Result of extracting known flags from a subcommand's argument list:
/// either help was requested, or the flags were parsed and some positional
/// arguments remain.
#[derive(Debug)]
enum Flags {
    Help,
    Parsed {
        recursive: bool,
        positionals: Vec<String>,
    },
}

/// Parses the arguments that follow the `storage` noun (e.g. `ls -r gs://...`).
pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> crate::Result<Command> {
    match args.next().as_deref() {
        Some("ls") => match extract_flags(args, "storage ls")? {
            Flags::Help => print_help_and_exit(LS_HELP),
            Flags::Parsed {
                recursive,
                positionals,
            } => {
                let uri = positionals.into_iter().next().ok_or_else(|| {
                    UsageError("storage ls: missing <gs://...> argument".into())
                })?;
                Ok(Command::Ls { uri, recursive })
            }
        },
        // `cat` has no flags of its own, so it needs its own explicit
        // -h/--help check rather than going through `extract_flags`.
        Some("cat") => match args.next() {
            Some(a) if is_help_flag(&a) => print_help_and_exit(CAT_HELP),
            Some(uri) => Ok(Command::Cat { uri }),
            None => Err(UsageError("storage cat: missing <gs://...> argument".into()).into()),
        },
        Some("cp") => match extract_flags(args, "storage cp")? {
            Flags::Help => print_help_and_exit(CP_HELP),
            Flags::Parsed {
                recursive,
                positionals,
            } => {
                let mut it = positionals.into_iter();
                let src = it
                    .next()
                    .ok_or_else(|| UsageError("storage cp: missing SRC argument".into()))?;
                let dst = it
                    .next()
                    .ok_or_else(|| UsageError("storage cp: missing DST argument".into()))?;
                Ok(Command::Cp { src, dst, recursive })
            }
        },
        Some("rm") => match extract_flags(args, "storage rm")? {
            Flags::Help => print_help_and_exit(RM_HELP),
            Flags::Parsed {
                recursive,
                positionals,
            } => {
                let uri = positionals.into_iter().next().ok_or_else(|| {
                    UsageError("storage rm: missing <gs://...> argument".into())
                })?;
                Ok(Command::Rm { uri, recursive })
            }
        },
        Some("-h") | Some("--help") | None => print_usage_and_exit(),
        Some(other) => Err(UsageError(format!("unknown `storage` subcommand `{other}`\n\n{USAGE}")).into()),
    }
}

fn is_help_flag(s: &str) -> bool {
    s == "-h" || s == "--help"
}

/// Pulls `-r`/`-R`/`--recursive` and `-h`/`--help` out of `args` (they may
/// appear anywhere, matching gcloud's argparse-style flag placement) and
/// returns either a help request or the recursive flag alongside the
/// remaining positional arguments, in order. Any other `-`-prefixed token
/// (other than a bare `-`, reserved for a future stdin/stdout convention) is
/// rejected rather than silently treated as a positional, since no other
/// `cp`/`ls`/`rm` flags are implemented yet.
fn extract_flags(args: impl Iterator<Item = String>, context: &str) -> crate::Result<Flags> {
    let mut recursive = false;
    let mut positionals = Vec::new();
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Flags::Help),
            "-r" | "-R" | "--recursive" => recursive = true,
            s if s.starts_with('-') && s != "-" => {
                return Err(UsageError(format!("{context}: unknown flag `{s}`")).into());
            }
            _ => positionals.push(arg),
        }
    }
    Ok(Flags::Parsed {
        recursive,
        positionals,
    })
}

fn print_usage_and_exit() -> ! {
    print!("{USAGE}");
    std::process::exit(0);
}

fn print_help_and_exit(text: &str) -> ! {
    print!("{text}");
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> impl Iterator<Item = String> {
        items
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn parsed(f: Flags) -> (bool, Vec<String>) {
        match f {
            Flags::Parsed {
                recursive,
                positionals,
            } => (recursive, positionals),
            Flags::Help => panic!("expected Flags::Parsed, got Flags::Help"),
        }
    }

    #[test]
    fn extracts_recursive_flag_anywhere() {
        let (r, pos) = parsed(extract_flags(args(&["gs://bucket", "-r"]), "t").unwrap());
        assert!(r);
        assert_eq!(pos, vec!["gs://bucket"]);

        let (r, pos) = parsed(extract_flags(args(&["-R", "gs://bucket"]), "t").unwrap());
        assert!(r);
        assert_eq!(pos, vec!["gs://bucket"]);

        let (r, pos) = parsed(extract_flags(args(&["--recursive", "src", "dst"]), "t").unwrap());
        assert!(r);
        assert_eq!(pos, vec!["src", "dst"]);
    }

    #[test]
    fn no_flag_means_not_recursive() {
        let (r, pos) = parsed(extract_flags(args(&["gs://bucket"]), "t").unwrap());
        assert!(!r);
        assert_eq!(pos, vec!["gs://bucket"]);
    }

    #[test]
    fn rejects_unknown_flags() {
        assert!(extract_flags(args(&["--bogus", "gs://bucket"]), "t").is_err());
    }

    #[test]
    fn recognizes_help_flag_anywhere() {
        assert!(matches!(
            extract_flags(args(&["-h"]), "t").unwrap(),
            Flags::Help
        ));
        assert!(matches!(
            extract_flags(args(&["gs://bucket", "--help"]), "t").unwrap(),
            Flags::Help
        ));
    }

    #[test]
    fn unknown_flag_is_a_usage_error() {
        let err = extract_flags(args(&["--bogus"]), "t").unwrap_err();
        assert!(err.downcast_ref::<UsageError>().is_some());
    }
}

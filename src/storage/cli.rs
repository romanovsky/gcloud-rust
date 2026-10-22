#[derive(Debug)]
pub(crate) enum Command {
    Ls { uri: String, recursive: bool },
    Cat { uri: String },
    Cp { src: String, dst: String, recursive: bool },
    Rm { uri: String, recursive: bool },
}

pub(crate) const USAGE: &str = "\
storage — a partial, dependency-free reimplementation of `gcloud storage`.

USAGE:
    gcloud-rust storage ls [-r|-R|--recursive] <gs://bucket[/prefix]>
    gcloud-rust storage cat <gs://bucket/object>
    gcloud-rust storage cp [-r|-R|--recursive] <src> <dst>   (local<->gs://, or gs://<->gs://)
    gcloud-rust storage rm [-r|-R|--recursive] <gs://bucket/object>

AUTH:
    Uses whatever account is active in `gcloud auth login` / `gcloud config`.

REQUIRES ON PATH:
    gcloud, curl, jq
";

/// Parses the arguments that follow the `storage` noun (e.g. `ls -r gs://...`).
pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> crate::Result<Command> {
    match args.next().as_deref() {
        Some("ls") => {
            let (recursive, positionals) = extract_recursive_flag(args, "storage ls")?;
            let uri = positionals
                .into_iter()
                .next()
                .ok_or("storage ls: missing <gs://...> argument")?;
            Ok(Command::Ls { uri, recursive })
        }
        Some("cat") => Ok(Command::Cat {
            uri: args.next().ok_or("storage cat: missing <gs://...> argument")?,
        }),
        Some("cp") => {
            let (recursive, positionals) = extract_recursive_flag(args, "storage cp")?;
            let mut it = positionals.into_iter();
            let src = it.next().ok_or("storage cp: missing SRC argument")?;
            let dst = it.next().ok_or("storage cp: missing DST argument")?;
            Ok(Command::Cp { src, dst, recursive })
        }
        Some("rm") => {
            let (recursive, positionals) = extract_recursive_flag(args, "storage rm")?;
            let uri = positionals
                .into_iter()
                .next()
                .ok_or("storage rm: missing <gs://...> argument")?;
            Ok(Command::Rm { uri, recursive })
        }
        Some("-h") | Some("--help") | None => print_usage_and_exit(),
        Some(other) => Err(format!("unknown `storage` subcommand `{other}`\n\n{USAGE}").into()),
    }
}

/// Pulls `-r`/`-R`/`--recursive` out of `args` (it may appear anywhere, matching
/// gcloud's argparse-style flag placement) and returns it alongside the
/// remaining positional arguments, in order. Any other `-`-prefixed token
/// (other than a bare `-`, reserved for a future stdin/stdout convention) is
/// rejected rather than silently treated as a positional, since no other
/// `cp`/`ls`/`rm` flags are implemented yet.
fn extract_recursive_flag(
    args: impl Iterator<Item = String>,
    context: &str,
) -> crate::Result<(bool, Vec<String>)> {
    let mut recursive = false;
    let mut positionals = Vec::new();
    for arg in args {
        match arg.as_str() {
            "-r" | "-R" | "--recursive" => recursive = true,
            s if s.starts_with('-') && s != "-" => {
                return Err(format!("{context}: unknown flag `{s}`").into());
            }
            _ => positionals.push(arg),
        }
    }
    Ok((recursive, positionals))
}

fn print_usage_and_exit() -> ! {
    print!("{USAGE}");
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

    #[test]
    fn extracts_recursive_flag_anywhere() {
        let (r, pos) = extract_recursive_flag(args(&["gs://bucket", "-r"]), "t").unwrap();
        assert!(r);
        assert_eq!(pos, vec!["gs://bucket"]);

        let (r, pos) = extract_recursive_flag(args(&["-R", "gs://bucket"]), "t").unwrap();
        assert!(r);
        assert_eq!(pos, vec!["gs://bucket"]);

        let (r, pos) = extract_recursive_flag(args(&["--recursive", "src", "dst"]), "t").unwrap();
        assert!(r);
        assert_eq!(pos, vec!["src", "dst"]);
    }

    #[test]
    fn no_flag_means_not_recursive() {
        let (r, pos) = extract_recursive_flag(args(&["gs://bucket"]), "t").unwrap();
        assert!(!r);
        assert_eq!(pos, vec!["gs://bucket"]);
    }

    #[test]
    fn rejects_unknown_flags() {
        assert!(extract_recursive_flag(args(&["--bogus", "gs://bucket"]), "t").is_err());
    }
}

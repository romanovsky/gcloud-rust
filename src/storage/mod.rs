mod auth;
pub(crate) mod cli;
mod commands;
mod http;
mod uri;

pub(crate) use cli::Command;

pub(crate) fn run(cmd: Command) -> crate::Result<()> {
    match cmd {
        Command::Ls { uri, recursive } => commands::ls(&uri, recursive),
        Command::Cat { uri } => commands::cat(&uri),
        Command::Cp {
            src,
            dst,
            recursive,
        } => commands::cp(&src, &dst, recursive),
        Command::Rm { uri, recursive } => commands::rm(&uri, recursive),
    }
}

/// Strips C0 control characters (including `\n`/`\r`/`\x1b`) from strings that
/// originate outside our control -- GCS object names and API error-body text
/// -- before they reach the terminal. Mirrors fd's `SanitizeErr`: without
/// this, an attacker-controlled object name containing an ANSI escape or
/// embedded newline could spoof terminal output.
pub(crate) fn sanitize(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize("a\u{1b}[31mb\nc\rd"), "a[31mbcd");
        assert_eq!(sanitize("plain text"), "plain text");
    }
}

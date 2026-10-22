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

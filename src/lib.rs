mod auth;
mod cli;
mod commands;
mod http;
mod uri;

/// Crate-wide error type. No external dependency needed: `std::io::Error`,
/// `FromUtf8Error`, `&str`, and `String` all convert into this via `?`/`.into()`.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn run() -> Result<()> {
    match cli::parse(std::env::args())? {
        cli::Command::StorageLs { uri, recursive } => commands::ls(&uri, recursive),
        cli::Command::StorageCat { uri } => commands::cat(&uri),
        cli::Command::StorageCp {
            src,
            dst,
            recursive,
        } => commands::cp(&src, &dst, recursive),
        cli::Command::StorageRm { uri, recursive } => commands::rm(&uri, recursive),
    }
}

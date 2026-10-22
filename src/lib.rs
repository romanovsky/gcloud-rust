mod cli;
mod storage;

/// Crate-wide error type. No external dependency needed: `std::io::Error`,
/// `FromUtf8Error`, `&str`, and `String` all convert into this via `?`/`.into()`.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn run() -> Result<()> {
    match cli::parse(std::env::args())? {
        cli::Command::Storage(cmd) => storage::run(cmd),
    }
}

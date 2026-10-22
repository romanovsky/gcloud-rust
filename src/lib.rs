mod cli;
mod storage;

/// Crate-wide error type. `anyhow::Error` boxes any `std::error::Error` and
/// preserves the full cause chain (via `.context()`/`.with_context()`),
/// unlike a bare `Box<dyn std::error::Error>` which only ever shows the
/// outermost message.
pub type Result<T> = anyhow::Result<T>;

/// Marker error for usage-stage failures (bad flags, missing positional
/// arguments, unknown command/subcommand) as opposed to runtime failures
/// (auth, network, API errors). `main.rs` downcasts for this type to choose
/// exit code 2 vs 1 -- the same `Box<dyn Error>`-downcast idiom zoxide uses
/// for its `SilentExit` marker, applied here via `anyhow::Error::downcast_ref`.
#[derive(Debug)]
pub struct UsageError(pub String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for UsageError {}

pub fn run() -> Result<()> {
    match cli::parse(std::env::args())? {
        cli::Command::Storage(cmd) => storage::run(cmd),
    }
}

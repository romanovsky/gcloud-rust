# gcloud-rust: adopting Rust CLI best practices (dependency-free)

## Constraint that shapes everything below

Confirmed empirically (repeated `curl`/`cargo` tests this session): `static.crates.io` returns HTTP 403
through the current network/proxy. **No crates.io dependency can be added** — not clap, not anyhow,
not thiserror, not lexopt, not textwrap. Every insight below had to be filtered through "what is the
underlying *pattern*, and can it be reproduced with only `std`?" rather than "which crate implements this."
If the network constraint is lifted later, several Phase-3 deferrals below become easy crate installs
instead of hand-rolled code — noted where relevant.

Research was done by two sub-agents fetching real source from GitHub (ripgrep, fd, bat, eza, zoxide,
starship, git-delta) and the Rust CLI book (rust-cli.github.io/book). Findings below are attributed at
repo level; specific PR numbers / line numbers came from the sub-agents' own fetches and were not
independently re-verified line-by-line, so treat those as "per research, go check the repo" rather than
audited fact.

---

## Research findings, by dimension

### 1. CLI argument processing

- **ripgrep** moved *off* clap onto `lexopt` (a minimal argument *lexer*, not a parser). Stated reasons
  (per research): shrink the dependency tree / improve compile times, and — the actual forcing function —
  clap couldn't cleanly express ripgrep's flag-override semantics (`-ttoml` must override an earlier
  `-Tlock`). They pay for it with a hand-built help generator, man-page generator, and completion generator.
- **fd** uses clap **derive** (`#[derive(Parser)] struct Opts`), flat struct, `ArgGroup` for mutually
  exclusive modes, negation flags via a paired hidden field + `overrides_with`.
- **bat** uses clap's **builder** API (not derive) with help text as inline string literals.
- **eza** currently uses clap too (migrated *to* it from a hand-rolled parser, specifically to stop
  maintaining two argument-handling code paths). The hand-rolled version it replaced (original **exa**)
  modeled flags as `Flag::Short(u8) | Long(&str)`, worked on `OsStr` (not `str`) so invalid-UTF-8 filenames
  survive round-tripping, and hand-tokenized clustered short flags (`-abx`).
- **starship** also uses clap derive, with the `Cli`/`Commands` struct living directly in `main.rs` — no
  dedicated `cli.rs` needed at that point in the project's growth.
- The reusable idea, independent of clap/lexopt: treat argv as a **token stream** (`Value`/`Short`/`Long`),
  where a flag that needs an argument explicitly calls "give me the next token" rather than a parser loop
  eyeballing `args[i+1]`. This is exactly what naive hand-rolled parsers get wrong (`--flag=value` syntax,
  and a `--` end-of-options terminator).
- **Concrete gap this surfaced in our code**: we use `std::env::args()`, which **panics** on non-UTF-8
  argv. `cp` accepts local filesystem paths, which can be non-UTF-8 on Unix. `std::env::args_os()` is the
  std-only fix for the *parsing* side (see Decision in Deferrals below re: how far we carry this).

### 2. Project structuring

- **zoxide**: one file per subcommand under `src/cmd/` (`add.rs`, `query.rs`, `remove.rs`, ...), a single
  `Cmd` enum (clap-derived) dispatched via one `match` into each subcommand's own `.run()`. This is
  essentially identical in shape to our existing `src/storage/commands/{ls,cat,cp,rm}.rs`.
- **starship**: no separate `cli.rs` at all — the top-level `Cli`/`Commands` struct lives in `main.rs`
  until the project outgrows that; dispatch sends each noun to its own module.
- **ripgrep**: workspace of many small crates, each owning one concern (matcher, searcher, printer, ignore
  rules, globbing); inside the main CLI crate, argument-handling has its own subtree (`flags/`) separate
  from search execution.
- **Takeaway for us**: our current shape (`src/cli.rs` thin dispatcher → `src/storage/{cli,mod,auth,http,uri}.rs`
  + `src/storage/commands/{ls,cat,cp,rm}.rs`) already matches the pattern these projects converge on at
  our scale. No structural change recommended here — see Phase 3 for the one deferred refinement
  (formalizing a `Run` trait like zoxide's) and why it's not worth doing yet.

### 3. Docs and user-facing messages

- **ripgrep**: per-flag help text lives *as a method* on the same `Flag` trait that defines the flag
  itself (`doc_short()`, `doc_long()`), not in a separate hand-maintained usage blob. Short help is
  manually column-aligned; long help uses a wrapping crate (unavailable to us) after stripping shared
  roff markup.
- **git-delta** (and clap generally): a flag's doc comment's **first line** becomes its `-h` summary; the
  **whole comment** becomes its `--help` detail. The maintainer explicitly modeled delta's own `-h`/`--help`
  split on ripgrep's, wanting help text "traveling with the executable" rather than living only in a README.
- **fd / bat**: both gate colored output on `std::io::IsTerminal` (stable std, no crate) and the `NO_COLOR`
  env var convention (`std::env::var_os("NO_COLOR")`), with the check written once and reused.
- **fd's `error.rs`**: a `SanitizeErr` trait strips control characters out of untrusted strings (e.g.
  filenames) before they reach the terminal, to prevent terminal-escape injection from attacker-controlled
  file/object names.
- **Rust CLI book**: recommends newline-delimited, type-tagged JSON for machine-readable output (citing
  ripgrep's `--json`) when that need arises.

### 4. Error handling

- **ripgrep**: `anyhow::bail!` plus a "did you mean" suggestion for unrecognized flags; stdout is
  lock-guarded before writing errors to stderr to avoid interleaving; three-tier exit codes
  (0 success/broken-pipe, 1 no-match, 2 usage/error).
- **fd**: propagates `anyhow::Error`, prints via the `{:#}` alternate form (full context, no backtrace
  noise); a dedicated `ExitCodes` enum including `KilledBySigint = 130` (re-raising the default SIGINT
  disposition rather than just `exit(130)`, so shell job control behaves correctly).
- **bat**: `thiserror`-derived enum error type.
- **zoxide**: notably **does not** hand-roll a custom error enum — plain `anyhow::Result` everywhere, plus
  one tiny `SilentExit { code }` marker error (blank `Display`) used purely to unwind to `main()` with a
  specific exit code without printing anything twice. `main()` detects it via
  `Box<dyn Error>::downcast::<SilentExit>()` — **this downcast is real, stable std**
  (`std::error::Error`, stable since 1.3.0), so the pattern ports directly to our `Box<dyn std::error::Error>`
  error type without anyhow.
- **Important correction caught while drafting this plan**: anyhow's `{:?}` prints the *whole* "Caused by:"
  chain automatically. Our `Box<dyn std::error::Error>` does not — `{}`/`{:?}` on it only ever shows the
  one error we're currently looking at. Adopting a `.context()`-style helper (below) is **inert** unless
  `main.rs` also explicitly walks `.source()` and prints each link. Both halves are one deliverable.
- **Bugs found by actually running our binary** (not inferred — see Verification below): broken-pipe
  handling is currently wrong, and per-subcommand `--help` doesn't work (two different ways).

### 5. Dealing with string literals

- Co-locating a flag's short/long help text with its *definition* (ripgrep's `Flag` trait methods;
  delta/clap's doc-comment convention) avoids the drift risk of a single hand-maintained `USAGE` const
  silently going stale as flags are added — the text and the parsing code are two unsynchronized sources
  of truth today.
- fd's control-character stripping is itself a "string literal hygiene" practice: never interpolate an
  untrusted string into terminal output unescaped.
- `NO_COLOR` / ANSI escape sequences, if we add them, should be named constants in one place, not scattered
  inline literals.
- Explicit scope check from research: ripgrep only built its templated help/doc system because it has
  ~100 flags. We have one real flag (`-r`) across four subcommands. **Do not import that machinery
  wholesale** — adopt the *shape* (help text next to the flag it documents) in a form proportional to our
  current size.

---

## Verification done before writing this plan (observed, not inferred)

Both suspected bugs were reproduced against the real binary before being written up as findings.

- **`storage cat` piped into a reader that closes early**: `cargo run -q -- storage cat
  gs://cloud-samples-data/video/cat.mp4 | head -c 1` → stderr prints `error: Broken pipe (os error 32)`,
  exit code **1**. (First attempt used a tiny CSV and showed exit 0 — our `cat` buffers the whole object
  before one `write_all`, so small objects finish writing before the reader closes. The bug only shows up
  once the object is bigger than the pipe buffer, confirmed using a real multi-MB video object from the
  public `cloud-samples-data` bucket.) Not a panic — Rust's runtime already treats `SIGPIPE` as an
  `io::ErrorKind::BrokenPipe` `Err`, not a crash. The fix is purely about *what main.rs does* with that
  specific error kind.
- **Per-subcommand `--help`, confirmed broken two different ways depending on the subcommand**:
  - `storage ls --help` / `storage cp --help` / `storage rm --help` → each prints
    `` error: storage <cmd>: unknown flag `--help` ``, exit 1. These three route through
    `extract_recursive_flag`, which rejects any `-`-prefixed token other than `-r`/`-R`/`--recursive`.
  - `storage cat --help` → prints `` error: not a gs:// URI: `--help` ``, exit 1. `cat`'s parsing has
    *no* flag-handling at all (`Ok(Command::Cat { uri: args.next().ok_or(...)? })`), so `--help` is
    swallowed as the positional URI argument and fails URI parsing instead — a different failure mode,
    same underlying gap. A fix must cover both shapes, not just `extract_recursive_flag`.

---

## Implementation plan

### Phase 1 — cheap, high-value, do first (bugs confirmed above)

1. **Fix broken-pipe handling.** In `src/main.rs`, before printing a top-level error, check whether it's
   an `io::Error` with `kind() == BrokenPipe` and exit `0` silently instead:
   ```rust
   if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
       if io_err.kind() == std::io::ErrorKind::BrokenPipe {
           std::process::exit(0);
       }
   }
   ```
   Works today with zero other changes: `storage::commands::cat`'s `write_all(&body)?` propagates the raw
   `io::Error` through `?` without being wrapped in a `format!(...)` string first, so it stays downcastable.
   This is the same "inspect the concrete type in `main`" idiom zoxide uses for `SilentExit` — reusable
   for item 3 below.

2. **Wire up per-subcommand `-h`/`--help` — two call sites, not one.** `storage::cli::parse`'s `ls`/`cp`/`rm`
   arms all go through `extract_recursive_flag`, which needs to recognize `-h`/`--help` and short-circuit
   to that subcommand's help instead of erroring. `cat`'s arm has no flag-handling at all and needs its own
   explicit check before treating the next argument as the URI. Don't fix only the shared helper and assume
   `cat` is covered — it isn't (see Verification above).

3. **Adopt a deliberate exit-code convention.** 0 = success, 1 = runtime/API error, 2 = usage error (bad
   flags/missing arguments). Simplest implementation: give `storage::cli::parse`'s and the root `cli::parse`'s
   error returns a distinguishable marker (e.g. wrap usage-stage errors in a small `UsageError(String)`
   newtype implementing `Error`/`Display`), and have `main.rs` `downcast_ref::<UsageError>()` to choose
   exit 2 vs 1 — same downcast idiom as item 1.

### Phase 2 — foundational, moderate effort

4. **`.context()`-style error chaining, std-only — two parts, both required:**
   - A small `Context` error type + `ResultExt::context(self, msg)` extension trait (≈20 lines), wrapping
     the original error as `.source()` rather than discarding it:
     ```rust
     struct Context { msg: String, source: Box<dyn std::error::Error> }
     impl std::error::Error for Context {
         fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(self.source.as_ref()) }
     }
     ```
   - **`main.rs` must walk the chain**, or the above is inert:
     ```rust
     eprintln!("error: {e}");
     let mut cause = e.source();
     while let Some(c) = cause {
         eprintln!("  caused by: {c}");
         cause = c.source();
     }
     ```
   Apply `.context()` incrementally at points that currently lose detail in a batch loop — e.g. wrap each
   `copy_one`/`fetch_object`/`put_object` failure inside `storage/commands/cp.rs`'s recursive loops with
   which file/object it was, on top of the lower-level curl failure message already produced in `http.rs`.

   **Interaction with Phase 1 item 1**: that item's `e.downcast_ref::<std::io::Error>()` broken-pipe check
   only works while the `BrokenPipe` error reaches `main` unwrapped, which is true today for `cat`'s
   `write_all(&body)?`. If `.context()` is later applied to that same call site, it gets boxed inside
   `Context` and the direct downcast stops matching. If that happens, update the broken-pipe check to walk
   `.source()` (the same chain-walking loop this item already adds) looking for `io::ErrorKind::BrokenPipe`
   at any link, not just at the top.

5. **Split `USAGE` into short + long forms**, enabling item 2 above properly: one line per subcommand for
   the top-level/`storage --help` view, a longer block per subcommand for `storage <cmd> --help`. Mirrors
   delta/clap's first-line-vs-whole-comment convention, done by hand with two string constants per
   subcommand instead of a doc-comment macro.

6. **TTY/`NO_COLOR`-gated minimal color** (std-only, `std::io::IsTerminal` is stable since Rust 1.70 —
   confirmed available, this toolchain is 1.99.1):
   ```rust
   fn use_color() -> bool {
       std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
   }
   ```
   Apply to the `error:` prefix at minimum. Cosmetic, cheap, low priority relative to items 1-5.

7. **Sanitize untrusted strings before printing** — object names from `storage ls` and any GCS error-body
   text we echo come from outside our control. A small `fn sanitize(s: &str) -> String` stripping C0
   control characters, applied at the print sites in `commands/ls.rs` and wherever an API error body is
   surfaced, closes the same class of terminal-injection risk fd's `SanitizeErr` guards against.

### Phase 3 — explicitly deferred (trigger-bound, not open questions)

- **Per-flag trait/registry (ripgrep's `Flag` trait, scaled down).** Deferred until we add a *second* real
  flag to any subcommand (e.g. `-n`/`--no-clobber` for `cp`). One flag doesn't justify the machinery; the
  research explicitly notes ripgrep only built theirs because they have ~100 flags.
- **Non-UTF-8 local path support (`OsString` plumbing).** **Decision, not an open question**: out of
  scope for now. `Command::Cp { src: String, dst: String, .. }` stays `String`; non-UTF-8 local filenames
  are unsupported. Revisit only if a real report surfaces — full `OsString` plumbing touches `cli.rs`,
  every `commands/*.rs`, and `uri.rs`, which isn't justified speculatively.
- **"Did you mean" fuzzy flag suggestions.** Low-priority polish; revisit only if time permits after
  Phase 1/2 land.
- **Formalize a `Run` trait across `commands/*`** (zoxide's shape). Deferred — our existing one-file-per-
  subcommand layout already matches the pattern; only worth the extra indirection if dispatch logic grows
  past a simple `match`.
- **Machine-readable `--format=json` for `ls`.** Deferred until explicitly requested. Natural extension
  point when asked (mirrors ripgrep's `--json`, the Rust CLI book's cited worked example).
- **Full ripgrep-style templated help/man-page/completion generation.** Not justified at four commands;
  the Phase 2 short/long split is the right-sized version of the same idea.
- **If the network block lifts**: `lexopt` and `clap` both become realistic again (lexopt is a much smaller
  dependency and maps closely onto the token-stream idiom already recommended above); `anyhow` would make
  Phase 2 item 4 unnecessary; `thiserror` would simplify any future custom error enum.

---

## Suggested order of work

Phase 1 (3 items) is small and fixes confirmed, reproducible bugs — do that first, independent of
everything else. Phase 2 is the bulk of the "adopt real patterns" work and should land as one coherent
pass (item 4's two halves must ship together). Phase 3 stays parked until its stated trigger occurs.

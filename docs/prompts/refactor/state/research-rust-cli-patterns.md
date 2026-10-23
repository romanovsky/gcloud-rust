# gcloud-rust: adopting Rust CLI best practices

## Constraint that shaped this doc originally — since resolved

This doc was originally written under the belief that **no crates.io dependency could be added**:
repeated `curl`/`cargo` tests had shown `static.crates.io` returning HTTP 403 through the corporate
proxy. Every finding below was filtered through "what's the underlying *pattern*, reproducible with only
`std`?" instead of "which crate implements this?", on that basis.

**That basis turned out to be wrong.** The 403 was real, but it meant "the public CDN is blocked on this
network," not "no dependency can be linked." That earlier development environment sat behind a network
that required pointing `[source.crates-io]` at a private registry mirror via a repo-local
`.cargo/config.toml`; that file has since been removed; this is a public repo and builds against the
plain public crates.io registry. See `STATE.md`'s dependency-constraint section for the current state.

This means most of the "if the network block lifts" notes originally scattered through Phase 3 below are
no longer hypothetical — they've been acted on. Rather than rewrite every paragraph below to erase the
dependency-free framing it was researched under (the underlying pattern research is still valid and still
the right way to think about *this* project's scale), each phase item and the Phase 3 list are annotated
with what was actually done. Treat the rest of this doc as "research done one way, partially executed a
better way once circumstances changed" rather than a stale artifact.

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
  std-only fix for the *parsing* side (see Decision in Deferrals below re: how far we carry this). **Still
  unaddressed** — not touched by this session's work; see Phase 3 (`OsString` plumbing is still an explicit
  non-goal, decision unchanged).

> **Decision made once dependencies became available (see Phase 3 below): clap was evaluated and
> explicitly rejected, not adopted.** The `-r`-is-overloaded-per-subcommand problem documented in
> `STATE.md` (recursive for `cp`/`ls`/`rm`, byte-range for `cat`, region for `sign_url`) is structurally
> the same class of problem that drove ripgrep off clap in the first place. `lexopt` remains a live,
> lower-risk option — not yet adopted, no trigger to justify it yet at one real flag.

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
  (formalizing a `Run` trait like zoxide's) and why it's not worth doing yet. **Unchanged by this session**
  — `storage/mod.rs` grew one small free function (`sanitize()`) but the module shape is identical.

### 3. Docs and user-facing messages

- **ripgrep**: per-flag help text lives *as a method* on the same `Flag` trait that defines the flag
  itself (`doc_short()`, `doc_long()`), not in a separate hand-maintained usage blob. Short help is
  manually column-aligned; long help uses a wrapping crate (unavailable to us) after stripping shared
  roff markup.
- **git-delta** (and clap generally): a flag's doc comment's **first line** becomes its `-h` summary; the
  **whole comment** becomes its `--help` detail. The maintainer explicitly modeled delta's own `-h`/`--help`
  split on ripgrep's, wanting help text "traveling with the executable" rather than living only in a README.
- **fd / bat**: both gate colored output on `std::io::IsTerminal` (stable std, no crate) and the `NO_COLOR`
  env var convention (`std::env::var_os("NO_COLOR")`), with the check written once and reused. **Done** —
  `main.rs::use_color()`, gating the `error:` prefix.
- **fd's `error.rs`**: a `SanitizeErr` trait strips control characters out of untrusted strings (e.g.
  filenames) before they reach the terminal, to prevent terminal-escape injection from attacker-controlled
  file/object names. **Done** — `storage::sanitize()`, applied to every object name `ls` prints and to GCS
  API error-body text (see New findings below for why the latter mattered more than expected).
- **Rust CLI book**: recommends newline-delimited, type-tagged JSON for machine-readable output (citing
  ripgrep's `--json`) when that need arises. Still deferred (Phase 3, unchanged).

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
  error type without anyhow. **This is exactly the shape we ended up shipping**, except once anyhow became
  available we used `anyhow::Error::downcast_ref` (same idea, anyhow's own implementation of the same
  pattern) for `UsageError` instead of a hand-rolled `Box<dyn Error>` version — see Phase 1 item 3 below.
- **Important correction caught while drafting this plan**: anyhow's `{:?}` prints the *whole* "Caused by:"
  chain automatically. Our original `Box<dyn std::error::Error>` did not — `{}`/`{:?}` on it only ever
  showed the one error we're currently looking at. Adopting a `.context()`-style helper (below) is **inert**
  unless `main.rs` also explicitly walks `.source()` and prints each link. Both halves are one deliverable.
  **This directly informed the real implementation**: see Phase 2 item 4 below — `crate::Result` now *is*
  `anyhow::Result`, so this correction became load-bearing rather than theoretical.
- **Bugs found by actually running our binary** (not inferred — see Verification below): broken-pipe
  handling was wrong, and per-subcommand `--help` didn't work (two different ways). **Both fixed** — see
  Phase 1.

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
  current size. **Followed**: `storage/cli.rs` got four flat `*_HELP` string constants, not a trait/macro
  system — proportional to one real flag.

---

## Verification done before writing this plan (observed, not inferred)

Both suspected bugs were reproduced against the real binary before being written up as findings, and both
have since been fixed and re-verified live (see "Implementation status" notes inline above and Phase 1
below) — this section is kept as-is as the historical record of how they were *found*.

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

## New findings from the ureq/anyhow migration (not part of the original research pass)

These surfaced while actually implementing the plan below once dependencies were unblocked — they're new
discoveries, not things the original dependency-free research could have anticipated.

- **ureq 3.x silently drops HTTP error response bodies by default.** `http_status_as_error` defaults to
  `true`, which collapses any 4xx/5xx into `Error::StatusCode(u16)` — just the number, no body. The
  original curl-based implementation explicitly used `--fail-with-body` to keep GCS's JSON error detail
  (`{"error":{"message": "The specified bucket does not exist."}}`), and a first-pass ureq port lost that
  silently (it still compiled and looked correct). Caught by checking ureq's vendored source
  (`error.rs`/`run.rs`/`config.rs`) rather than by a live failure, then confirmed fixed against a real 404.
  Fix: `.config().http_status_as_error(false).build()` per request, then inspect `resp.status()` and read
  the body manually in all cases. Lesson for future crate adoptions in this project: **check what a crate
  silently drops by default before trusting its error `Display` is sufficient** — it rarely carries as
  much detail as a hand-rolled curl invocation that was explicitly tuned for this API's error format.
- **`ureq::Response` is `http::Response<ureq::Body>`, not ureq's own type** — the `http` crate had to be
  added as an explicit direct `Cargo.toml` dependency (not just relying on it transitively through ureq)
  because ureq doesn't re-export it. Minor, but worth knowing before assuming "we only added ureq."
- **Two real `ls -r` formatting gaps**, found by live-diffing against real `gcloud storage ls -r` at bucket
  shapes that hadn't been tested before (bucket-root, and a prefix that's itself a folder-placeholder
  object) — full detail in `STATE.md`'s Known bugs section. Both fixed and covered by new unit tests in
  `storage/commands/ls.rs` (`bucket_root_gets_its_own_header`, `placeholder_object_gets_bare_top_line`,
  `no_placeholder_means_no_bare_top_line`). Mentioned here because it's a second instance of the same
  lesson as above: **the only way these were found was diffing real output at shapes not previously
  exercised** — code review / pattern research alone would not have surfaced either.

---

## Implementation plan

### Phase 1 — cheap, high-value, do first (bugs confirmed above) — ✅ ALL DONE

1. **Fix broken-pipe handling.** ✅ **Done**, with one change from the originally-planned shape: because
   Phase 2 item 4 below wraps errors in `anyhow::Context` (not left as bare `io::Error` at the top level),
   a single top-level `downcast_ref::<io::Error>()` would stop matching once `.context()` is attached
   anywhere upstream. Implemented as a **chain walk** instead, from the start, rather than shipping the
   naive version and fixing it later:
   ```rust
   fn is_broken_pipe(e: &anyhow::Error) -> bool {
       e.chain()
           .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
           .any(|io_err| io_err.kind() == std::io::ErrorKind::BrokenPipe)
   }
   ```
   Confirmed live: `cargo run -- storage cat gs://cloud-samples-data/video/cat.mp4 | head -c1` → exit 0,
   silent. Same "inspect the concrete type in `main`" idiom zoxide uses for `SilentExit`, now via
   `anyhow::Error::chain()` instead of a single `Box<dyn Error>::downcast_ref`.

2. **Wire up per-subcommand `-h`/`--help` — two call sites, not one.** ✅ **Done.** The shared flag
   extractor (renamed `extract_flags`, was `extract_recursive_flag`) now returns an enum
   (`Flags::Help | Flags::Parsed { .. }`) instead of erroring on `-h`/`--help`, used by `ls`/`cp`/`rm`.
   `cat`'s arm (no flag loop at all) got its own explicit check before treating the next token as the URI,
   exactly as flagged — fixing only the shared helper would *not* have covered it. Confirmed live for all
   four subcommands; each now prints its own long-form help text (see Phase 2 item 5).

3. **Adopt a deliberate exit-code convention.** ✅ **Done.** 0 = success, 1 = runtime/API error, 2 = usage
   error (bad flags/missing arguments) — scoped exactly as planned, to `storage::cli::parse`'s and the root
   `cli::parse`'s error returns only (not to deeper validation in `uri.rs` or `commands/*.rs`, which stay
   plain runtime errors / exit 1). Implemented via a `UsageError(String)` marker type in `lib.rs`
   (`Display`/`Error` impls, wraps into `anyhow::Error` via the blanket `From` impl), downcast in
   `main.rs` via `anyhow::Error::downcast_ref::<UsageError>()` — same idiom as item 1, now using anyhow's
   own downcast instead of `Box<dyn Error>`'s. Confirmed live: unknown flag / missing arg / unknown
   subcommand → exit 2; bad URI / nonexistent bucket → exit 1.

### Phase 2 — foundational, moderate effort — ✅ ALL DONE, via anyhow instead of hand-rolled code

Originally planned as std-only (no crates available at the time). Once the registry block lifted,
item 4 was built directly on `anyhow` instead — exactly the substitution this doc's own
Phase 3 "if the network block lifts" note predicted for this item specifically; applied here, once deps
were confirmed working, rather than waiting.

4. **Error chaining — anyhow, not a hand-rolled `Context` type.** ✅ **Done.** `crate::Result<T>` is now
   `anyhow::Result<T>` (`lib.rs`). Every error-construction site across the crate was migrated from
   `Err("...".into())` / `.ok_or("...")?` (relying on std's `impl From<&str> for Box<dyn Error>`, which
   anyhow deliberately does *not* provide) to `anyhow::bail!`/`anyhow!`/`.context()`/`.with_context()`.
   `main.rs` walks the chain and prints each link:
   ```rust
   eprintln!("error: {e}");
   for cause in e.chain().skip(1) {
       eprintln!("  caused by: {cause}");
   }
   ```
   (`.skip(1)` because `e.chain()`'s first item is the top-level error already shown via `{e}`.)
   `.with_context()` applied exactly where the original plan called for it — `cp.rs`'s `put_object`/
   `fetch_object`/`copy_one` calls inside the recursive batch loops, plus the equivalent `delete_one` loop
   in `rm.rs`, each wrapped with which file/object it was, on top of the lower-level `http.rs` failure
   message. Per-item failures in those batch loops are collected via `format!("{e:#}")` — anyhow's
   alternate Display, which prints the full chain on one line (fd's convention, noted in the original
   research) — rather than a hand-built, single-level string.

5. **Split `USAGE` into short + long forms.** ✅ **Done**, enabling item 2 above. `storage::cli::USAGE`
   is the short form (top-level/`storage --help`, and the tail of "unknown subcommand" errors);
   `LS_HELP`/`CAT_HELP`/`CP_HELP`/`RM_HELP` are one constant each, shown by `storage <cmd> --help`. Hand-
   written constants, not a doc-comment macro — proportional to one real flag, per the research's own
   explicit scope check (dimension 5 above).

6. **TTY/`NO_COLOR`-gated minimal color.** ✅ **Done**, exactly as planned:
   ```rust
   fn use_color() -> bool {
       std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
   }
   ```
   Applied to the `error:` prefix in `main.rs`. Still cosmetic/low-priority, as originally scoped.

7. **Sanitize untrusted strings before printing.** ✅ **Done**, and turned out to matter more than
   expected. `storage::sanitize()` (strips C0 control chars) is applied to every object name `ls` prints
   (both the non-recursive and the grouped recursive formatter) **and** to GCS API error-body text now
   that `http.rs` actually surfaces that text (see New findings above — before that fix, there was no
   error-body text to sanitize in the first place, since it was being discarded).

### Phase 3 — revisited now that dependencies are available; decisions made, not just deferred

- **Per-flag trait/registry (ripgrep's `Flag` trait, scaled down).** Still deferred until we add a
  *second* real flag to any subcommand. Unchanged.
- **clap.** ✅ **Evaluated and explicitly rejected** (this is a decision, not an oversight or a "not yet").
  Reasoning: real `gcloud storage`'s `-r` is overloaded per-subcommand (recursive for `cp`/`ls`/`rm`,
  byte-range for `cat`, region for `sign_url` — confirmed against the actual Python SDK source, see
  `STATE.md`'s Prior Research section) in exactly the way that drove ripgrep itself off clap (dimension 1
  above). Adopting clap now would also put at risk the byte-exact `ls -r` output parity against real
  `gcloud` that was just established via live diffing. The hand-rolled parser stays.
- **lexopt.** Still an open, not-yet-adopted option — lower risk than clap since it's a lexer rather than
  an opinionated parser (would preserve our own per-subcommand validation), but not justified yet at one
  real flag. Revisit alongside the per-flag-registry trigger above, together, if either fires.
- **anyhow.** ✅ **Adopted** — see Phase 2 item 4. No longer hypothetical.
- **thiserror.** Still not adopted, and not clearly needed: the one custom error type we have
  (`UsageError`) is a single-field marker, trivial to hand-write `Display`/`Error` for. thiserror's value
  is deriving those across several enum variants — revisit if a real multi-variant error enum shows up.
- **Non-UTF-8 local path support (`OsString` plumbing).** Still out of scope, decision unchanged.
  `Command::Cp { src: String, dst: String, .. }` stays `String`. Note from dimension 1 above: this is now
  slightly more clearly a real (if narrow) gap, not just a theoretical one, since `std::env::args()` does
  panic on non-UTF-8 argv — still not prioritized without a real report.
- **"Did you mean" fuzzy flag suggestions.** Still deferred, unchanged.
- **Formalize a `Run` trait across `commands/*`** (zoxide's shape). Still deferred, unchanged — dispatch
  is still a simple `match`.
- **Machine-readable `--format=json` for `ls`.** Still deferred until explicitly requested, unchanged.
- **Full ripgrep-style templated help/man-page/completion generation.** Still not justified at four
  commands, unchanged.

---

## Suggested order of work

Original ordering (Phase 1 first, Phase 2 as one coherent pass, Phase 3 parked) was followed as planned —
**Phase 1 and Phase 2 are both complete**, Phase 2 landed as the anyhow-based version of the originally
std-only plan rather than two separate passes, and Phase 3 now has explicit per-item decisions instead of
blanket deferral. Remaining open items are exactly the still-deferred Phase 3 bullets above (none of their
stated triggers have fired) plus the two things called out in `STATE.md`: `.cargo/config.toml`/
`Cargo.lock` need committing, and `cp`/`rm` still need a real (disposable) bucket to verify live.

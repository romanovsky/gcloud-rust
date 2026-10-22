# gcloud-rust — project state (for resuming work)

Read this before re-deriving anything from conversation history. Companion doc in this same folder:
`research-rust-cli-patterns.md` (full research + phased implementation plan — this file is the index/
quick-orientation layer on top of it).

## What this project is

A partial reimplementation of `gcloud storage` in Rust. Goal (see `README.md`): replace the
Python-based `gcloud` CLI with a fast, reliable Rust CLI. **No longer dependency-free** — see below,
this changed mid-project and invalidated the original constraint the research doc was written under.

## Dependency constraint — RESOLVED, was previously blocking

Earlier sessions confirmed `static.crates.io` (the public crates.io CDN) returns HTTP 403 through the
corporate proxy, and concluded no crates.io dependency could be linked. **That conclusion was wrong** —
it conflated "the public CDN is blocked" with "no dependency can be added." Walmart runs an Artifactory
proxy for crates.io, documented at
[Cargo usage](https://confluence.walmart.com/pages/viewpage.action?pageId=3207638393). Pointing
`[source.crates-io]` at it unblocks normal dependency downloads entirely:

```toml
# .cargo/config.toml
[source.crates-io]
registry = "sparse+https://cargo.ci.artifacts.walmart.com/artifactory/api/cargo/rust-lang-cargo-release-remote/index/"
```

**⚠️ `.cargo/config.toml` and `Cargo.lock` are untracked in git as of this writing** (`git status`
shows both `??`). Source + `README.md` were committed (`4375875`), but without these two files a fresh
clone cannot build — `cargo build` will hit the same 403 this section just described. Commit both
before considering this done. (Flagged twice now — don't lose it a third time.)

The original 403 finding itself was real and reproducible (confirmed independently via a direct `curl`
against `static.crates.io`, with and without Cargo in the loop) — it just didn't mean what it was taken
to mean. If `.cargo/config.toml` is ever missing/misconfigured, that exact symptom returns:

```sh
curl -sS -o /dev/null -w "%{http_code}\n" https://static.crates.io/crates/cfg-if/1.0.5/download
# 403 == proxy not configured / not taking effect; 200 would mean the raw CDN itself opened up
```

## Curl/jq/gcloud shell-outs — removed, replaced with crates

Now that deps work, all three external-binary shell-outs the project used to depend on are gone:

| Was | Now |
|---|---|
| `curl` (HTTP) | `ureq` 3.5 |
| `jq` (JSON parsing) | `serde_json` + `serde` |
| `gcloud auth print-access-token` (OAuth2) | Native ADC file read + direct OAuth2 refresh call |

**Auth detail, corrected from an earlier wrong assumption**: the native path reads
`~/.config/gcloud/application_default_credentials.json` (or `$GOOGLE_APPLICATION_CREDENTIALS`), which
is populated by `gcloud auth application-default login` — **not** `gcloud auth login`. Those are two
separate credential stores. If the ADC file is missing/malformed, `storage/auth.rs` falls back to
shelling out to `gcloud auth print-access-token`, which *does* need plain `gcloud auth login`. (An
earlier version of this doc and the CLI's own `--help` text incorrectly said the native path needed
`gcloud auth login` — fixed in both places.)

**Both paths are now actually exercised, not just plausible on paper**: confirmed live with
`GOOGLE_APPLICATION_CREDENTIALS=/nonexistent ./target/debug/gcloud-rust storage ls gs://cloud-samples-data/adc/`
— ADC read fails, falls through to `gcloud auth print-access-token`, succeeds, correct listing returned.
Also fixed a real error-swallow caught on review: the fallback match arm used to be `Err(_) =>
token_from_gcloud()`, discarding the ADC failure reason entirely — if gcloud *also* failed, the user only
ever saw the gcloud error with no clue the ADC path was even tried. Now the gcloud attempt is wrapped with
`.with_context()` carrying the original ADC error inline, so a double-failure (confirmed live with both
`GOOGLE_APPLICATION_CREDENTIALS` pointed at a bogus path and `gcloud` removed from `PATH`) prints both:
```
error: could not get an access token: ADC failed (could not read /nonexistent: ...), and the gcloud fallback also failed
  caused by: failed to run `gcloud`. Is the Google Cloud SDK installed and on PATH?
  caused by: No such file or directory (os error 2)
```

`gcloud` itself is therefore now only a fallback dependency, not a hard requirement, and `curl`/`jq` are
gone entirely — `README.md`'s Requirements section reflects this.

**A real bug this surfaced**: ureq 3.x's default behavior (`http_status_as_error: true`) collapses any
4xx/5xx response into `Error::StatusCode(u16)` — the response **body is discarded**. The original curl
implementation explicitly used `--fail-with-body` to keep GCS's JSON error detail
(`{"error":{"message": "..."}}`), and the first ureq port silently lost that. Fixed in `storage/http.rs`
by setting `.config().http_status_as_error(false).build()` per request and reading status+body manually.
Confirmed against a real 404 (`gs://this-bucket-definitely-does-not-exist-xyz123/`) that the full GCS
error body now surfaces, sanitized through the same `sanitize()` helper used for object names.

`http` had to be added as an explicit direct dependency (`Cargo.toml`) — `ureq`'s `Response<Body>` is
`http::Response<ureq::Body>`, and ureq does not re-export the `http` crate.

## Prior research — don't re-derive, and don't get this wrong

Before the CLI-patterns research (below), a separate pass read the real `gcloud` Python SDK source
(present on this machine at `/Users/s0r0mzn/Downloads/gs/google-cloud-sdk/lib/surface/storage/` and
`.../googlecloudsdk/command_lib/storage/`) to establish ground truth for `gcloud storage`'s actual flag
surface. Two load-bearing facts from that pass:

- **`-r` is overloaded and means something different per subcommand** — recursive for `cp`/`ls`/`rm`/
  `rsync`, but **byte range** for `cat` (`-r 256-939`) and **region** for `sign_url`; `du`'s `-r` means
  readable-sizes, not recursive. This is the specific reason a derive-macro parser (clap) was rejected
  when dependencies became available (see research doc) — clap models one meaning per flag name
  globally, not per-subcommand-overloaded. `mv` is always-recursive internally with no flag at all.
- Real `gcloud storage cp` has **~25+ flags**; our implementation covers exactly one (`-r`/`-R`/`--recursive`)
  deliberately — see Phase 3 of the research doc for when/how to grow that surface.
- The recursive-`cp` **nest-vs-rename destination rule** we implemented (does `cp -r dir gs://b/dest` land
  at `dest/dir/...` or `dest/...`?) was not guessed — it was read directly out of
  `copy_task_iterator.py::_get_destination_suffix_for_recursion` in the SDK source above and matched with
  unit tests in `src/storage/commands/cp.rs`.

## Current structure

```
src/
  main.rs, lib.rs, cli.rs      — thin top-level dispatcher (Command::Storage(...))
  storage/
    mod.rs                      — pub(crate) Command, run() dispatch, sanitize() helper
    cli.rs                      — storage subcommand parsing (Ls/Cat/Cp/Rm), short USAGE +
                                   per-subcommand LS_HELP/CAT_HELP/CP_HELP/RM_HELP
    auth.rs, http.rs, uri.rs
    commands/{mod,ls,cat,cp,rm}.rs
```

Deliberately shaped so a future command group (e.g. a `compute` noun) can be added as a sibling of
`storage/` without touching storage's internals — one more match arm in root `cli.rs`/`lib.rs`, one more
`print!` line in the top-level usage. `README.md` now has a Status table listing every other gcloud
command group as not-started, for exactly this reason.

## What's implemented vs. actually verified live

| Command | Implemented | Verified live |
|---|---|---|
| `storage ls` / `ls -r` | yes, with `nextPageToken` pagination, gcloud-matching group headers | **yes** |
| `storage cat` | yes | **yes** |
| `storage cp` (upload / download / gs↔gs, recursive + not) | yes | **no** |
| `storage rm` / `rm -r` | yes | **no** |

Safe, read-only, re-runnable smoke tests against public buckets (no credentials beyond the local
`gcloud auth login` / `gcloud auth application-default login` session needed):

```sh
cargo run -q -- storage ls gs://cloud-samples-data/
cargo run -q -- storage ls -r gs://cloud-samples-data/bigquery/us-states/
cargo run -q -- storage cat gs://cloud-samples-data/bigquery/us-states/us-states.csv
```

**`ls -r` output format is now byte-for-byte diffed against real `gcloud storage ls -r`**, not just
spot-checked, across three distinct real-bucket shapes:

1. A prefix with no folder-placeholder object (`gs://adtech-prod-offsite-ydsp-ingress/drop_zone/impressions/`,
   253 lines including headers) — exact match.
2. A prefix that *is* itself a folder-placeholder object (`gs://cloud-samples-data/bigquery/us-states/`) —
   exact match, including the extra bare top-line + blank line gcloud emits in this case (see Known bugs,
   now fixed, below).
3. True bucket root (empty prefix) — **not** diffed byte-for-byte live (the bucket used,
   `cloud-samples-data`, is too large to fully enumerate inside a reasonable tool-call timeout — this
   implementation collects every page before printing anything, so a `head`-truncated live run produces
   zero output on our side long before gcloud's equivalent would, which is a timing artifact, not a
   correctness signal). Instead verified by: (a) directly observing that real
   `gcloud storage ls -r gs://cloud-samples-data/` does emit its own `gs://cloud-samples-data/:` root
   header before descending (`timeout 20 ... | head -4`, confirmed), and (b) a deterministic unit test
   (`bucket_root_gets_its_own_header` in `commands/ls.rs`) proving the formatter does the same for an
   empty prefix. Don't upgrade this to "live-diffed" without actually doing it against a smaller
   bucket-rooted scenario.

**Standing caveat — don't lose this**: `cp` and `rm` have never been run against a real bucket end to end;
this environment has no disposable/writable bucket. The single highest-risk unverified detail (whether
`%2F`-encoded slashes in the `name=`/`prefix=` query parameters round-trip correctly on upload) *was*
independently confirmed correct via a direct call against live GCS — see the research doc — but the
actual write/delete calls themselves are implemented-but-unexercised. Don't upgrade this to "verified"
without actually running them against a bucket someone owns.

`cargo build`, `cargo test` (23 tests, up from 15), and `cargo clippy` all pass clean as of this state.

## Known bugs — Phase 1's three are now FIXED (were open at last write-up)

1. ~~`storage cat <large-object> | head` → `error: Broken pipe (os error 32)`, exit 1~~ **Fixed.**
   `main.rs` now walks the *whole* anyhow error chain (`e.chain().filter_map(downcast_ref::<io::Error>())`)
   looking for `ErrorKind::BrokenPipe` at any link, not just a top-level downcast — deliberately chain-aware
   because Phase 2's `.context()`/`.with_context()` adoption would otherwise wrap the original `io::Error`
   and silently break a naive top-level-only check (this exact interaction was flagged before being
   implemented, not discovered after). Confirmed live: `storage cat gs://cloud-samples-data/video/cat.mp4
   | head -c1` → exit 0, nothing on stderr.
2. ~~Per-subcommand `--help` broken two different ways~~ **Fixed**, both ways, separately:
   `extract_flags` (used by `ls`/`cp`/`rm`) now returns a `Flags::Help` sentinel instead of erroring on
   `-h`/`--help`; `cat`'s arm (which has no flag loop at all) got its own explicit check before treating
   the next token as the URI. Confirmed live for all four subcommands.

**Two new, previously-unknown `ls -r` formatting bugs were found (and fixed) while verifying the above**
(not inferred — found by live-diffing against real `gcloud storage ls -r` on public buckets, which hadn't
been done at bucket-root / placeholder-object shapes before):

3. Bucket-root recursive listing (`storage ls -r gs://bucket/`, empty prefix) was silently skipping its
   own `gs://bucket/:` header — the code special-cased "don't emit a header for an empty prefix," which
   turned out to be wrong by inductive testing against three different non-empty prefixes plus a direct
   check against real bucket-root output. Fixed by removing the special case; the formatting rule is just
   "the queried prefix always gets a header," full stop.
4. When the queried prefix is *itself* a literal object name (a GCS folder-placeholder/marker object —
   common when a directory was created via the GCS console or `gsutil mkdir`-equivalent tooling), real
   `gcloud storage ls -r` prints that object once more as a bare line (no trailing `:`) plus a blank line,
   *before* any group headers. Confirmed against `gs://cloud-samples-data/bigquery/` and
   `.../bigquery/us-states/`. Fixed by detecting `entries.iter().any(|e| e == prefix)` and emitting the
   extra line only in that case — confirmed via unit test that it's a no-op when no such placeholder exists
   (the far more common case, e.g. the adtech bucket this was first verified against).

Exit-code convention (0 success / 1 runtime error / 2 usage error) is also now implemented, via a
`UsageError` marker type (`lib.rs`) that `storage::cli::parse` and root `cli::parse` wrap usage-stage
failures in, downcast in `main.rs` — same idiom zoxide uses for its `SilentExit` marker, now using
`anyhow::Error::downcast_ref` instead of a bare `Box<dyn Error>`'s.

## Known limitation, newly surfaced (not a regression, not yet fixed)

`storage ls -r` fully enumerates every page of results (following `nextPageToken` to exhaustion) and
builds the entire formatted output as one `String` *before* printing anything. For a prefix with a huge
number of objects (observed trying to use true bucket-root on `gs://cloud-samples-data/` — likely
hundreds of thousands of objects bucket-wide) this means no output at all appears until every page has
been fetched, unlike real `gcloud` which appears to stream. This was already true before this session's
changes (pre-existing in the original `list_entries`/print loop shape) and is not something Phase 1/2/3
of the research doc called for fixing — noted here only because it was surfaced while verifying item 3
above, and because it blocks doing a full live diff at true-bucket-root scale on a large bucket. Not
scheduled; would need `list_entries` and the formatter restructured around streaming/incremental printing
if it's ever prioritized.

## Next steps

Full detail in `research-rust-cli-patterns.md`. Summary, now that it's been executed:

- **Phase 1 — done.** All three items (broken pipe, per-subcommand help, exit codes) fixed and verified
  live, per Known bugs above.
- **Phase 2 — done, via anyhow instead of hand-rolled std-only code**, since dependencies are available
  now (this is exactly the doc's own "if the network block lifts" Phase-3 prediction for item 4, now
  acted on for the whole phase, not just that one item):
  - Item 4 (error chaining): `anyhow::Context`/`bail!`/`anyhow!` throughout, replacing the planned
    hand-rolled `Context` struct. `.with_context()` applied at `cp.rs`/`rm.rs`'s batch-loop call sites
    (`put_object`/`fetch_object`/`copy_one`/`delete_one`) and at every `http.rs` call, exactly where the
    doc called out detail was being lost. Failures in batch loops print via `{:#}` (anyhow's alternate
    Display, full inline chain) rather than a flattened hand-built string.
  - Item 5 (short/long help split): done — `storage::cli::USAGE` (short) plus `LS_HELP`/`CAT_HELP`/
    `CP_HELP`/`RM_HELP` (long, one per subcommand).
  - Item 6 (TTY/NO_COLOR color): done, gates the `error:` prefix in `main.rs`.
  - Item 7 (sanitize untrusted strings): done — `storage::sanitize()` strips C0 control chars, applied to
    every object name printed by `ls` and to GCS API error-body text surfaced by `http.rs`.
- **Phase 3 — revisited now that deps are available, decisions made (not just deferred):**
  - **clap: explicitly rejected for now**, not just "not done yet." Reasoning: `-r` is overloaded
    per-subcommand in real `gcloud storage` (recursive for most, byte-range for `cat`, region for
    `sign_url` — see Prior Research above) in a way clap's per-flag-name model doesn't cleanly express,
    which was the exact historical reason ripgrep itself moved *off* clap onto `lexopt`. Adopting it now
    would also risk the byte-exact `ls -r` output parity that was just established against real `gcloud`.
    Keep the hand-rolled parser.
  - **lexopt: still an open option, not adopted.** Lower risk than clap (it's a lexer, not an opinionated
    parser — preserves our own validation logic), but not justified yet at one real flag across four
    subcommands. Revisit alongside the per-flag-registry trigger below.
  - **thiserror: not adopted.** No multi-variant custom error enum exists yet (`UsageError` is a single
    one-field marker type, trivial to hand-write); thiserror's value is deriving `Error`/`Display` across
    several variants. Revisit if a real enum shows up.
  - Everything else in the original Phase 3 list (per-flag registry, `OsString` plumbing, "did you mean"
    suggestions, a `Run` trait, JSON output for `ls`) is still deferred, unchanged from the original
    research doc — none of their triggers have occurred.

## Repo / git state

As of this write-up: source (`src/`), `Cargo.toml`, and `README.md` are committed (`4375875`,
"refactor + update docs"). **`.cargo/config.toml` and `Cargo.lock` are still untracked** (`git status`
shows `??` for both) — see the dependency-constraint section above for why this is blocking for anyone
else cloning the repo. `.idea/` is also untracked and should probably be gitignored rather than committed.
Ask before committing, per standing instruction.

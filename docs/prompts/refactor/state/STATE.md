# gcloud-rust — project state (for resuming work)

Read this before re-deriving anything from conversation history. Companion doc in this same folder:
`research-rust-cli-patterns.md` (full research + phased implementation plan — this file is the index/
quick-orientation layer on top of it).

## What this project is

A partial, **dependency-free** reimplementation of `gcloud storage` in Rust. "Dependency-free" is not a
style choice — see the constraint below.

## Environment constraint — re-check before assuming it's stale

`cargo build` / `cargo fetch` cannot download any crate. The corporate proxy returns HTTP 403 on
`static.crates.io` for every package download (crate *metadata* via the sparse index works fine —
`cargo add` itself succeeds — only the actual `.crate` tarball download is blocked). Confirmed repeatedly
this session, independent of Cargo:

```sh
curl -sS -o /dev/null -w "%{http_code}\n" https://static.crates.io/crates/cfg-if/1.0.5/download
```

A `200` here means the block has lifted and `clap`/`anyhow`/etc. become viable again — see the "if lifted"
note at the end of `research-rust-cli-patterns.md` before changing the design on that basis.

Because of this, the binary shells out to three external programs instead of linking crates:

- `gcloud auth print-access-token` — auth (`src/storage/auth.rs`)
- `curl` — all HTTP to the GCS JSON API; bearer token is passed via a `-K -` stdin config, never argv, so
  it doesn't leak into `ps` output (`src/storage/http.rs`)
- `jq` — JSON parsing of list responses (`src/storage/commands/ls.rs`)

All three were confirmed present and working on this machine.

## Prior research — don't re-derive, and don't get this wrong

Before the CLI-patterns research (above), a separate pass read the real `gcloud` Python SDK source
(present on this machine at `/Users/s0r0mzn/Downloads/gs/google-cloud-sdk/lib/surface/storage/` and
`.../googlecloudsdk/command_lib/storage/`) to establish ground truth for `gcloud storage`'s actual flag
surface. Two load-bearing facts from that pass:

- **`-r` is overloaded and means something different per subcommand** — recursive for `cp`/`ls`/`rm`/
  `rsync`, but **byte range** for `cat` (`-r 256-939`) and **region** for `sign_url`; `du`'s `-r` means
  readable-sizes, not recursive. A future session must not add `-r` to `cat` expecting recursion — it
  would contradict real `gcloud storage` semantics. `mv` is always-recursive internally with no flag at all.
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
    mod.rs                      — pub(crate) Command, run() dispatch
    cli.rs                      — storage subcommand parsing (Ls/Cat/Cp/Rm), USAGE text
    auth.rs, http.rs, uri.rs
    commands/{mod,ls,cat,cp,rm}.rs
```

Deliberately shaped so a future command group (e.g. a `compute` noun) can be added as a sibling of
`storage/` without touching storage's internals — one more match arm in root `cli.rs`/`lib.rs`, one more
`print!` line in the top-level usage. (Rationale cross-checked against real-world Rust CLI project layouts
— see dimension 2 in the research doc.)

## What's implemented vs. actually verified live

| Command | Implemented | Verified live |
|---|---|---|
| `storage ls` / `ls -r` | yes, with `nextPageToken` pagination | **yes** |
| `storage cat` | yes | **yes** |
| `storage cp` (upload / download / gs↔gs, recursive + not) | yes | **no** |
| `storage rm` / `rm -r` | yes | **no** |

Safe, read-only, re-runnable smoke tests against a public bucket (no credentials beyond the local
`gcloud auth login` session needed):

```sh
cargo run -q -- storage ls gs://cloud-samples-data/
cargo run -q -- storage ls -r gs://cloud-samples-data/bigquery/us-states/
cargo run -q -- storage cat gs://cloud-samples-data/bigquery/us-states/us-states.csv
```

**Standing caveat — don't lose this**: `cp` and `rm` have never been run against a real bucket end to end;
this environment has no disposable/writable bucket. The single highest-risk unverified detail (whether
`%2F`-encoded slashes in the `name=`/`prefix=` query parameters round-trip correctly on upload) *was*
independently confirmed correct via a direct `curl` call against live GCS — see the research doc — but the
actual write/delete calls themselves are implemented-but-unexercised. Don't upgrade this to "verified"
without actually running them against a bucket someone owns.

`cargo build` and `cargo test` (15 tests across 3 suites) both pass clean as of this state.

## Known bugs (confirmed by running the binary — full repro detail in the research doc)

1. `storage cat <large-object> | head` → `error: Broken pipe (os error 32)`, exit 1, instead of a clean
   exit 0. Only manifests once the object is bigger than the OS pipe buffer (a small test object hides it).
2. Per-subcommand `--help` is broken, two different ways depending on the subcommand — see the research
   doc's Verification section for the exact two error strings and why `cat` fails differently from
   `ls`/`cp`/`rm`.

Both are Phase 1 items below.

## Next steps

Full detail in `research-rust-cli-patterns.md`. Summary:

- **Phase 1 (small, do first)**: fix the two bugs above (note: the `--help` fix needs two separate code
  changes, not one); adopt a 0/1/2 exit-code convention (success / runtime error / usage error) via the
  same `downcast_ref::<T>()`-on-the-boxed-error idiom used for the broken-pipe fix.
- **Phase 2 (moderate)**: a std-only `.context()` error-chaining helper, **shipped together with** a
  `main.rs` change that walks `.source()` to print the chain (the helper alone is inert); split `USAGE`
  into short/long forms to make Phase 1's per-subcommand help possible; `std::io::IsTerminal` + `NO_COLOR`
  gated color on error output; sanitize untrusted strings (GCS object names, API error bodies) before
  printing them.
- **Phase 3 (explicitly deferred, trigger-bound — not open questions)**: a per-flag registry (defer until
  a 2nd real flag is added anywhere); `OsString`/non-UTF-8 local path support (decided out of scope,
  `String` stays as-is, revisit only on a real report); fuzzy "did you mean" flag suggestions; a `Run`
  trait across `commands/*`; JSON output for `ls`.

## Repo / git state

Working tree has uncommitted changes on top of commit `020034e`. Nothing from this session has been
committed — ask before committing, per standing instruction.

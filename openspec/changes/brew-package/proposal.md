## Why

`gcloud-rust` currently has exactly one install path: clone the repo and `cargo build`. That's fine for
development but works against the project's own stated goal (README: "fast, reliable Rust CLI" replacing
Python `gcloud`) — a tool meant to replace a one-command `gcloud` install shouldn't require a Rust
toolchain and a git clone to try. Homebrew is the standard install path for CLI tools on macOS, and it's
how the comparable Rust CLI tools this project's own design research is modeled on (ripgrep, fd, bat,
eza, zoxide) are distributed.

Shipping via Homebrew surfaced a blocking, pre-existing problem worth fixing regardless: this repo's
committed `.cargo/config.toml` unconditionally redirects crate downloads to an internal-only registry
proxy, reachable only from inside one company's network. That host is unreachable from outside that
network — including GitHub Actions, which any binary-release pipeline needs. Confirmed on `origin/main`: the file is
committed as-is, and `Cargo.lock` being committed alongside it does not help, since cargo still fetches
`.crate` files from whatever registry `[source.crates-io]` names. Public CI cannot build this repo today.

## What Changes

- Add a `--version`/`-V` flag to the CLI (currently has none) — needed so the Homebrew formula's
  standard `test do` block has something to assert against, and so users/support can tell what they have
  installed.
- **BREAKING for zero-config builds on that internal network**: remove the repo-committed
  `.cargo/config.toml` registry override. A public, externally-buildable repo can't ship a config file
  that only works inside one company's network. Contributors on that network will need their own local override (same
  one-line snippet, moved to an uncommitted `~/.cargo/config.toml` or a gitignored repo-local one) —
  documented in `README.md` and `STATE.md` so this isn't a silent regression for the existing workflow.
- Add a GitHub Actions release workflow (`.github/workflows/release.yml`): on pushing a `v*` tag, build
  release binaries for macOS (`aarch64-apple-darwin`, `x86_64-apple-darwin`), package each as a checksummed
  tarball, and publish them as GitHub Release assets.
- Tag and publish the project's first release, `v0.1.0`, through that pipeline.
- Update `README.md` with Homebrew install instructions and a note on the release process.
- Out of scope for *this repo*, called out explicitly rather than silently dropped: the actual Homebrew
  tap (a separate `homebrew-gcloud-rust` GitHub repo containing the Formula) lives outside this
  repository's `allowedEditRoots` and outside this change's tasks. `design.md` specifies the Formula this
  project's release assets must support; creating that tap repo is a follow-up change.
- Explicitly not doing, recorded as assumptions rather than open questions: submitting to `homebrew-core`
  (the project is a partial, 0.1.0, single-command-group reimplementation — doesn't meet homebrew-core's
  notability/stability bar yet), and Linux binaries (v1 is macOS-only; Homebrew-on-Linux is a documented
  future extension, not a non-goal forever).

## Capabilities

### New Capabilities
- `homebrew-distribution`: the release pipeline and CLI behavior needed for `gcloud-rust` to be
  installable and verifiable via a Homebrew formula — version reporting, the public-buildability
  prerequisite, and the tagged-release artifact contract the (separate) tap's Formula depends on.

### Modified Capabilities
(none — no existing capability specs exist in this project yet; this is the first)

## Impact

- `src/cli.rs` / `src/main.rs`: new `--version`/`-V` handling.
- `.cargo/config.toml`: removed from the repo (currently committed; see BREAKING note above).
- `README.md`, `docs/prompts/refactor/state/STATE.md`: updated with the relocated registry-override
  guidance, install instructions, and release process.
- New: `.github/workflows/release.yml`.
- New: a `v0.1.0` git tag and its corresponding GitHub Release + binary assets.
- Outside this repo: a `homebrew-gcloud-rust` tap repo (follow-up change, not built here).

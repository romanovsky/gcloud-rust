## Context

See `proposal.md` - Why/What Changes for motivation. Relevant current state:

- The repo is public on GitHub (`romanovsky/gcloud-rust`), with no `.github/workflows/` yet and no git
  tags — there has never been a release.
- `Cargo.toml` is at `version = "0.1.0"`. The CLI has no `--version`/`-V` handling today.
- `.cargo/config.toml` is committed on `origin/main` (confirmed) and unconditionally overrides
  `[source.crates-io]` to an internal-only registry proxy. Confirmed this breaks `cargo build` for
  anyone/anything without access to that host — including GitHub Actions runners, which this change
  depends on. `Cargo.lock` being committed doesn't change this: cargo still fetches `.crate` files from
  whatever registry `[source.crates-io]` resolves to.
- `gh` CLI is available locally; dev machine is macOS arm64 (`aarch64-apple-darwin`).

## Goals / Non-Goals

**Goals:**
- Produce checksummed, runnable macOS release binaries automatically from a pushed version tag.
- Make the repository buildable by something with no access to that internal network (a prerequisite
  for the above, not a separate feature).
- Give the CLI a `--version` flag so both the release pipeline and the (future) Homebrew formula's test
  block have something to verify against.

**Non-Goals:**
- Writing the actual Homebrew Formula / tap repo. That repo doesn't exist, lives outside this
  repository's edit root, and depends on this change's release assets existing first — it's a follow-up
  change, not a task here. This change produces what that Formula will point at.
- Linux binaries. Homebrew-on-Linux is real, but v1 targets macOS only (Homebrew's primary platform, and
  the only platform this project has been built/tested on so far). Adding a Linux target later is
  additive to the CI matrix, not a breaking change to anything built here.
- Submitting to `homebrew-core`. A partial, 0.1.0, single-command-group reimplementation doesn't meet its
  notability/stability bar; revisit once the project covers more of `gcloud`'s surface.
- Redesigning how developers on that internal network fetch dependencies in general. This change only moves the
  registry override from committed to developer-local; it doesn't change the proxy itself or how it
  was discovered (see `STATE.md`'s existing dependency-constraint section).

## Decisions

**Prebuilt binaries over build-from-source formula.** Chosen per explicit product decision: faster
`brew install` (no Rust toolchain needed on the end user's machine), and it matches how this project's
own CLI-patterns research (`research-rust-cli-patterns.md`) already characterized comparable tools
(ripgrep, fd, bat, eza, zoxide all ship prebuilt). Alternative considered: a source-build formula
(`cargo install` under the hood) — simpler, needs no CI at all — rejected because it's a worse end-user
experience and inconsistent with the tools this project already treats as its reference points.

**macOS only (`aarch64-apple-darwin` + `x86_64-apple-darwin`) for v1.** Both targets build on a single
`macos-14`-class GitHub Actions runner: Xcode's bundled linker supports cross-linking both Darwin
architectures from one toolchain, so `rustup target add x86_64-apple-darwin` plus a second
`cargo build --release --target ...` is sufficient — no second runner OS, no Docker/`cross` needed (that
tool is for cross-*OS* targets, e.g. Linux from a non-Linux host; not needed for Darwin-on-Darwin).

**Remove the committed `.cargo/config.toml` entirely; no "conditional" version.** Considered keeping a
conditional override (e.g., only apply the registry redirect when some env var is set) — not
achievable: cargo's `[source.crates-io]` replacement in `.cargo/config.toml` is static TOML, resolved the
same way for every invocation that sees the file; there's no built-in "fall back to the public registry if
the replacement is unreachable." The only way a public repo builds cleanly outside that internal network is to
not ship a universal override in the repo at all. Contributors on that network (including this
machine, going forward) need the same one-line snippet in their own `~/.cargo/config.toml` (global, not
repo-local, so it's never at risk of being committed again) — documented in `README.md` and `STATE.md`.

**`--version` reads `env!("CARGO_PKG_VERSION")` at compile time**, not a hand-maintained string constant —
guarantees the binary's reported version always matches the `Cargo.toml` version it was built from (and
therefore the release tag, since the tag/version bump happens together — see Migration Plan), with no
separate place to forget to update.

**Release automation via tag-triggered GitHub Actions + `gh release` (CLI already available locally for
manual dry-runs, but CI uses the standard `actions/upload-release-asset`-equivalent step, not a local
`gh` invocation).** Checksums via `shasum -a 256` on each tarball, published as a sibling `.sha256` file —
this is exactly what a Homebrew Formula's `sha256` field needs, so the follow-up tap change can copy
values directly rather than recomputing them.

## Risks / Trade-offs

- **[Risk]** Removing `.cargo/config.toml` could silently break `cargo build` for any other
  contributor on that internal network who doesn't see the doc update → **Mitigation**: the failure mode (403 from
  `static.crates.io`) is already diagnosed and documented in `STATE.md`'s existing dependency-constraint
  section; the fix is copy-pasting the same snippet to `~/.cargo/config.toml`, documented in both
  `README.md` and `STATE.md` as part of this change.
- **[Risk]** Cross-compiling `x86_64-apple-darwin` from an `aarch64` GitHub runner is standard practice
  today but depends on Xcode's toolchain continuing to support it → **Mitigation**: a CI failure here is
  caught at tag-push time, before any formula references the broken asset; falling back to a dedicated
  x86_64 runner is a one-line workflow change if it's ever needed.
- **[Risk]** This is the project's first-ever release — no prior art for the tag/release process in this
  repo → **Mitigation**: dry-run with a pre-release tag (`v0.1.0-rc1`) first; delete the dry-run tag and
  its release after confirming the pipeline works, before cutting the real `v0.1.0`.
- **[Trade-off]** Prebuilt binaries mean a release isn't "done" until the full CI matrix succeeds (vs.
  build-from-source, which can't fail to release, only be slow to install) — accepted as standard for this
  class of tool.
- **[Trade-off]** macOS-only v1 means Homebrew-on-Linux users can't install yet. Documented as a known gap
  in `README.md`'s Status section rather than silently omitted; not a regression since no install path
  exists for them today either.

## Migration Plan

1. Add `--version`/`-V` handling; verify locally.
2. Remove `.cargo/config.toml` from git tracking; add the local-override instructions to
   `README.md` and `STATE.md`; verify `cargo build` still works on this machine (uses the existing warm
   `~/.cargo/registry` cache plus a newly-added personal global override).
3. Add `.github/workflows/release.yml`.
4. Push a dry-run pre-release tag (`v0.1.0-rc1`) to validate the pipeline end-to-end (binaries build,
   checksums generated, Release published with correct assets); delete the dry-run tag + Release once
   confirmed.
5. Tag and push `v0.1.0` for real; confirm the Release and both binary+checksum asset pairs are published.
6. Update `README.md`: release/version-check process, and an explicit note that Homebrew installation is
   pending the (separate, follow-up) tap repo — do not publish a `brew install` line that would 404 today.

**Rollback**: this is a packaging-only change with no runtime system affected. If the release pipeline
misbehaves after a real tag is pushed, delete the GitHub Release and the tag, fix forward, and re-tag —
nothing downstream depends on a `gcloud-rust` release existing yet.

## 1. CLI version flag

- [ ] 1.1 Add top-level `--version`/`-V` handling (checked before subcommand dispatch, no auth/network
      call needed) using `env!("CARGO_PKG_VERSION")`; verify `cargo run -- --version` and
      `cargo run -- -V` both print `gcloud-rust 0.1.0` and exit 0 with no network activity.
- [ ] 1.2 Add a unit test asserting the printed format; verify `cargo test` passes.
- [ ] 1.3 Regression-check existing dispatch: verify `storage ls|cat|cp|rm --help`/`-h` and normal
      `storage` subcommand invocations still behave exactly as before (the version check sits at the
      top-level parser shared by all of them) — rerun the existing `cargo test` suite (23 tests) plus a
      couple of live smoke commands from `STATE.md`'s safe-smoke-test list.

## 2. Remove the internal-only registry override from the repo

- [ ] 2.1 `git rm .cargo/config.toml`; verify `cargo build` still succeeds locally (existing warm
      `~/.cargo/registry` cache from prior work on this machine).
- [ ] 2.2 Add the same `[source.crates-io]` snippet to this machine's own `~/.cargo/config.toml`
      (global, outside the repo, not committed); verify `cargo clean && cargo build` succeeds end-to-end
      (forces a real re-fetch through the new global override, proving it's not just cache warmth
      masking a broken config).
- [ ] 2.3 Update `README.md`'s Requirements section and `STATE.md`'s dependency-constraint section to
      describe the relocated, developer-local override instead of the removed repo file; verify by
      confirming neither doc references a committed `.cargo/config.toml` anymore.

## 3. Release automation (CI)

- [ ] 3.1 Add `.github/workflows/release.yml`, triggered on tag push matching `v*`, with a build matrix
      over `aarch64-apple-darwin` and `x86_64-apple-darwin` targets on a `macos-14`-class runner
      (`rustup target add` the non-native one, then `cargo build --release --target <target>` for each);
      verify the workflow passes GitHub's own syntax validation on push (Actions tab shows no parse
      error).
- [ ] 3.2 In the same workflow, package each target's binary as `gcloud-rust-<target>.tar.gz` and write
      its SHA256 to a matching `<file>.sha256`; verify by code review (can't fully verify until a tag is
      actually pushed — see group 4).
- [ ] 3.3 Publish both files per target as GitHub Release assets for the pushed tag; verify once the
      dry-run tag in task 4.1 actually runs end-to-end.

## 4. Dry-run and first real release

- [ ] 4.1 Push a pre-release tag `v0.1.0-rc1`; verify the Actions run succeeds and the Release page shows
      4 assets (tarball + checksum × 2 platforms).
- [ ] 4.2 Download the asset matching this machine's platform, verify its SHA256 against the published
      checksum file, extract it, and run `./gcloud-rust --version`; verify it exits 0 and prints
      `gcloud-rust 0.1.0` (`Cargo.toml`'s current version — this dry run intentionally doesn't bump it,
      since its purpose is validating pipeline mechanics, not the version string itself).
- [ ] 4.3 Delete the `v0.1.0-rc1` tag and its GitHub Release (dry-run cleanup); verify via `git tag -l`
      and the repo's Releases page that neither remains.
- [ ] 4.4 Tag and push `v0.1.0` for real; verify the Release is published with both platforms'
      checksummed assets, same checks as task 4.2, and that the printed version now matches the real
      tag.

## 5. Documentation

- [ ] 5.1 Update `README.md` with the release/version-check process, and an explicit note that Homebrew
      installation is pending a separate tap repository (follow-up change) rather than publishing a
      `brew install` line that would 404 today; verify by reading the rendered section.
- [ ] 5.2 Update `STATE.md`'s Next steps with the follow-up, out-of-this-repo work needed to actually
      ship the Homebrew tap/Formula (new `homebrew-gcloud-rust` repo, Formula referencing this change's
      release assets — see `design.md`'s Decisions for the exact shape); verify by reading the updated
      section back.

## 6. Final verification

- [ ] 6.1 Run `cargo test` and `cargo clippy --all-targets`; verify both are clean (0 failures,
      0 warnings), matching this project's existing quality bar.
- [ ] 6.2 Confirm every requirement in `specs/homebrew-distribution/spec.md` has a corresponding
      verified task above (version flag → 1.1; public buildability → 2.2; tagged release artifacts →
      3.1-3.3, 4.1, 4.4; runnable+versioned release binary → 4.2; Homebrew end-to-end requirement is
      explicitly deferred to the follow-up tap change per `design.md` Non-Goals — note this explicitly
      rather than silently leaving it unchecked).

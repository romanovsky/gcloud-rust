## 1. CLI version flag

- [x] 1.1 Add top-level `--version`/`-V` handling (checked before subcommand dispatch, no auth/network
      call needed) using `env!("CARGO_PKG_VERSION")`; verify `cargo run -- --version` and
      `cargo run -- -V` both print `gcloud-rust 0.1.0` and exit 0 with no network activity.
      Verified: both print `gcloud-rust 0.1.0`, exit 0 (`src/cli.rs`).
- [x] 1.2 Add a unit test asserting the printed format; verify `cargo test` passes.
      Verified: `version_line_is_name_plus_cargo_pkg_version` in `src/cli.rs`; 24/24 tests pass.
- [x] 1.3 Regression-check existing dispatch: verify `storage ls|cat|cp|rm --help`/`-h` and normal
      `storage` subcommand invocations still behave exactly as before (the version check sits at the
      top-level parser shared by all of them) — rerun the existing `cargo test` suite (23 tests) plus a
      couple of live smoke commands from `STATE.md`'s safe-smoke-test list.
      Verified: top-level `-h`, `storage -h`, and `ls|cat|cp|rm --help` all unchanged; live smoke test
      (`storage ls gs://cloud-samples-data/`) unchanged. Note: piping `storage ls` into `head -3`
      panics with a broken-pipe error from the `println!` macro — confirmed via `git stash` that this
      reproduces identically without this change's diff, so it's a pre-existing gap (unrelated to the
      version flag), not a regression. Flagging, not fixing — out of scope for this task.

## 2. Remove the internal-only registry override from the repo

- [x] 2.1 `git rm .cargo/config.toml`; verify `cargo build` still succeeds locally (existing warm
      `~/.cargo/registry` cache from prior work on this machine).
      Already done in commit `11004eb` (prior session's internal-reference-removal pass). Verified: file
      absent from the repo, `cargo build` succeeds.
- [x] 2.2 Add the same `[source.crates-io]` snippet to this machine's own `~/.cargo/config.toml`
      (global, outside the repo, not committed); verify `cargo clean && cargo build` succeeds end-to-end
      (forces a real re-fetch through the new global override, proving it's not just cache warmth
      masking a broken config).
      Already done in prior session. Re-verified: `cargo clean && cargo build --release` forced a full
      579 MiB re-fetch/rebuild through `~/.cargo/config.toml`, succeeded in 16.58s. Caveat: this proves
      the build works *on this machine, through the internal registry override* — it is not evidence for
      spec Requirement 2 ("builds without access to any internal-only registry"), which only a registry
      with no access to that internal network (i.e. a public CI runner, group 4) can actually demonstrate.
      See 6.2.
- [x] 2.3 Update `README.md`'s Requirements section and `STATE.md`'s dependency-constraint section to
      describe the relocated, developer-local override instead of the removed repo file; verify by
      confirming neither doc references a committed `.cargo/config.toml` anymore.
      Already done in commit `11004eb`. Verified: `README.md` Requirements section and `STATE.md`'s
      "Dependency constraint — resolved, no longer applicable" section both describe the personal,
      uncommitted `~/.cargo/config.toml` override; neither references a committed repo file.

## 3. Release automation (CI)

- [x] 3.1 Add `.github/workflows/release.yml`, triggered on tag push matching `v*`, with a build matrix
      over `aarch64-apple-darwin` and `x86_64-apple-darwin` targets on a `macos-14`-class runner
      (`rustup target add` the non-native one, then `cargo build --release --target <target>` for each);
      verify the workflow passes GitHub's own syntax validation on push (Actions tab shows no parse
      error).
      Added. Verified structure locally (Ruby/Psych YAML parse: `jobs: build, release`; trigger
      `push.tags: ["v*"]`; matrix targets correct) since the workflow hasn't been pushed yet — GitHub's
      own syntax check is still pending the paused group 4 (needs explicit go-ahead; see session notes).
- [x] 3.2 In the same workflow, package each target's binary as `gcloud-rust-<target>.tar.gz` and write
      its SHA256 to a matching `<file>.sha256`; verify by code review (can't fully verify until a tag is
      actually pushed — see group 4).
      Done by code review: `tar -czf` + `shasum -a 256` steps per matrix leg, uploaded as build artifacts.
- [x] 3.3 Publish both files per target as GitHub Release assets for the pushed tag; verify once the
      dry-run tag in task 4.1 actually runs end-to-end.
      Done by code review: `release` job downloads all matrix artifacts (`merge-multiple: true`) and
      runs `gh release create <tag> ... dist/*`, with `permissions: contents: write` for the token.
      Used `gh release create` (uploads files directly) instead of design.md's originally-cited
      `actions/upload-release-asset`, which is deprecated/unmaintained — functionally equivalent, same
      asset-publishing outcome. End-to-end run still pending group 4.

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

- [x] 5.1 Update `README.md` with the release/version-check process, and an explicit note that Homebrew
      installation is pending a separate tap repository (follow-up change) rather than publishing a
      `brew install` line that would 404 today; verify by reading the rendered section.
      Added new `## Releases` section (tag trigger, `--version` check, explicit "not available yet via
      Homebrew" note with reason) plus a one-line macOS-only platform note under `## Status`, per
      design.md's trade-off note. No `brew install` line published.
- [x] 5.2 Update `STATE.md`'s Next steps with the follow-up, out-of-this-repo work needed to actually
      ship the Homebrew tap/Formula (new `homebrew-gcloud-rust` repo, Formula referencing this change's
      release assets — see `design.md`'s Decisions for the exact shape); verify by reading the updated
      section back.
      Added a "Homebrew distribution — in progress" bullet to Next steps: done-so-far, what's paused in
      this repo and why (gh auth host mismatch + publishing-needs-go-ahead), and what's out-of-repo
      (tap Formula) vs. not planned (homebrew-core, Linux).

## 6. Final verification

- [x] 6.1 Run `cargo test` and `cargo clippy --all-targets`; verify both are clean (0 failures,
      0 warnings), matching this project's existing quality bar.
      Verified: 24/24 tests pass, clippy clean, after all changes (version flag + workflow + docs).
- [x] 6.2 Confirm every requirement in `specs/homebrew-distribution/spec.md` has a corresponding
      verified task above (version flag → 1.1; public buildability → 2.2; tagged release artifacts →
      3.1-3.3, 4.1, 4.4; runnable+versioned release binary → 4.2; Homebrew end-to-end requirement is
      explicitly deferred to the follow-up tap change per `design.md` Non-Goals — note this explicitly
      rather than silently leaving it unchecked).

      Requirement-by-requirement status:
      - **CLI reports its own version** → 1.1/1.2. Fully verified: `--version`/`-V` print
        `gcloud-rust 0.1.0`, exit 0, unit-tested.
      - **Builds without access to any internal-only registry** → 2.1 (file removed), 2.2 (local
        verification only). **Not fully verified yet**: 2.2's `cargo clean && cargo build` proves the
        build works *through this machine's internal registry override*, which is the opposite of what
        this requirement claims. The only real proof is a registry with no access to that internal
        network actually building it — i.e. the `release.yml` workflow (3.1) succeeding on GitHub's own
        `macos-14` runners. That run is part of the paused group 4.
      - **Tagged releases produce checksummed macOS binaries** → 3.1-3.3 done by code review (workflow
        written, YAML structure validated locally); end-to-end proof needs 4.1/4.4, both paused.
      - **Release binaries are runnable and self-report matching version** → covered by 4.2, paused.
      - **Installing via Homebrew yields a working, correctly versioned binary** → explicitly deferred
        to the follow-up tap-repo change, per `design.md` Non-Goals (that repo doesn't exist and is
        outside this change's `allowedEditRoots`). Not claimed as satisfied here.

      Net: 1 of 5 requirements fully verified (version flag); 1 partially (public buildability — code
      done, proof pending CI); 2 blocked on the paused group 4 (tagged-release artifacts, runnable
      release binary); 1 explicitly out of scope for this repo (Homebrew end-to-end).

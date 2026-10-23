## Purpose

Defines the version-reporting, public-buildability, and release-artifact behavior that `gcloud-rust`
must provide so it can be installed and verified through a Homebrew formula, instead of requiring a
source build.

## ADDED Requirements

### Requirement: CLI reports its own version
The CLI SHALL support a `--version` flag (and its short form `-V`) that prints the running binary's
version and exits successfully, independent of any subcommand.

#### Scenario: user requests the version
- **WHEN** the binary is run with `--version` (or `-V`)
- **THEN** it prints `gcloud-rust <version>` to stdout, where `<version>` matches the crate version of
  the build, and exits with status 0

### Requirement: the project builds without access to any internal-only registry
A clean checkout of the repository SHALL build successfully (`cargo build --release`) using only
the public crates.io registry, with no dependency on network endpoints reachable from inside one
company's network only.

#### Scenario: build from a machine with no access to the internal registry host
- **WHEN** `cargo build --release` is run from a fresh clone, on a machine that cannot reach
  the internal-only registry host
- **THEN** the build completes successfully, resolving dependencies through the public crates.io
  registry

### Requirement: tagged releases produce checksummed macOS binaries
Pushing a version tag SHALL produce a published GitHub Release containing a binary archive and a
SHA256 checksum for each supported target platform.

#### Scenario: a version tag is pushed
- **WHEN** a tag matching `v*` (e.g. `v0.1.0`) is pushed to the repository
- **THEN** a GitHub Release for that tag is published with a binary tarball and a `.sha256` checksum
  file for each of `aarch64-apple-darwin` and `x86_64-apple-darwin`

### Requirement: release binaries are runnable and self-report the matching version
A binary published in a tagged release SHALL run on its target platform without additional runtime
dependencies and SHALL report the same version as its release tag.

#### Scenario: a release binary is downloaded and run
- **WHEN** a release's binary asset for the host's platform is downloaded, extracted, and executed
  with `--version`
- **THEN** it exits 0 and prints the version matching the release tag it came from

### Requirement: installing via Homebrew yields a working, correctly versioned binary
Installing the published Homebrew formula on a supported macOS platform SHALL result in a `gcloud-rust`
binary on `PATH` that reports the installed release's version.

#### Scenario: a user installs via Homebrew
- **WHEN** a user on a supported macOS platform (arm64 or x86_64) runs `brew install gcloud-rust` via
  the project's tap
- **THEN** `gcloud-rust --version` succeeds and reports the version of the formula that was installed

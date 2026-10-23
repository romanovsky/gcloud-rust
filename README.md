# gcloud-rust

A partial reimplementation of `gcloud` in Rust.

## Goal

Replace the Python-based `gcloud` CLI with a fast, reliable Rust CLI — same commands, a fraction
of the startup latency and no Python runtime/venv to manage.

See [docs/Performance-testing-results.md](docs/Performance-testing-results.md) for measurements against
real `gcloud` (~160x faster startup, ~3-10x faster end-to-end depending on command).

## Requirements

- Rust / Cargo (edition 2024)
- One of:
  - `gcloud auth application-default login` run once (preferred — read natively via ADC + OAuth2
    refresh, no `gcloud` binary needed at runtime), or
  - `gcloud auth login` with the `gcloud` CLI on `PATH` (fallback: shells out to
    `gcloud auth print-access-token`)

HTTP and JSON are handled natively (`ureq`, `serde_json`) — no `curl`/`jq` shell-outs. Dependencies
come from the public crates.io registry; no special `.cargo/config.toml` is needed to build this repo.

## Build

```sh
cargo build
```

## Test

```sh
cargo test
```

## Run

```sh
cargo run -- storage ls gs://bucket[/prefix]
cargo run -- storage ls -r gs://bucket[/prefix]       # recursive, grouped output (matches `gcloud storage ls -r`)
cargo run -- storage cat gs://bucket/object
cargo run -- storage cp <src> <dst>                    # local<->gs://, or gs://<->gs://
cargo run -- storage rm gs://bucket/object
```

Run `cargo run -- storage <command> --help` for per-command flag details.

## Releases

Pushing a `v*` tag (e.g. `v0.1.0`) triggers `.github/workflows/release.yml`, which builds, checksums,
and publishes macOS binaries (`aarch64-apple-darwin`, `x86_64-apple-darwin`) as GitHub Release assets.
Every binary reports its own version:

```sh
gcloud-rust --version   # or -V
```

**Homebrew install isn't available yet** — it depends on a separate `homebrew-gcloud-rust` tap
repository (outside this repo) that hasn't been created yet. Until then, either build from source (see
Build above) or grab a release tarball from the
[Releases page](https://github.com/romanovsky/gcloud-rust/releases), verify its `.sha256`, and put the
extracted binary on `PATH`.

## Status

`gcloud` has ~135 top-level command groups. Only `storage` is covered so far.

| Command group | Status |
|---|---|
| `storage` | 🟡 Partial — `ls`, `cat`, `cp`, `rm` (see [Run](#run)) |

Release binaries (see [Releases](#releases)) currently target macOS only (`aarch64-apple-darwin`,
`x86_64-apple-darwin`); Linux is a possible future addition, not implemented today.

### Not started

Everything else, including the commonly-used groups:

`compute`, `container`, `functions`, `run`, `sql`, `pubsub`, `iam`, `projects`, `config`, `auth`,
`logging`, `monitoring`, `builds`, `artifacts`, `app`, `dns`, `kms`, `secrets`, `scheduler`,
`deploy`, `spanner`, `bigtable`, `firestore`, `datastore`, `dataflow`, `dataproc`, `composer`,
`endpoints`, `source`, `workflows`, `batch`, `redis`, `filestore`, `services`, `billing`,
`resource-manager`, `organizations`, `org-policies`, `identity`, `bq`

and the rest of the surface (access management, networking sub-groups, data/AI products, and
niche/alpha products) — run `gcloud --help` for the complete list.

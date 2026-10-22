# gcloud-rust

A partial, dependency-free reimplementation of `gcloud storage`.

## Requirements

- Rust / Cargo
- `gcloud` (authenticated: `gcloud auth login`)
- `curl`
- `jq`

No crates.io dependencies — the binary shells out to `gcloud`, `curl`, and `jq` instead of linking an HTTP/auth crate.

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
cargo run -- storage cat gs://bucket/object
cargo run -- storage cp <src> <dst>   # local<->gs://, or gs://<->gs://
cargo run -- storage rm gs://bucket/object
```

# Performance testing results

Informal wall-clock comparison of `gcloud-rust` against vanilla `gcloud` (Python), run on one machine
in one network session on 2026-10-22. Not a rigorous benchmark suite — no `hyperfine`, no warm/cold
isolation beyond what's noted inline, single tester, single point in time. Treat the ratios as "roughly
this much faster," not as guaranteed numbers for any other machine, network, or bucket.

**Masking legend**: all real bucket/object paths below are replaced with `gs://BUCKET/prefix/...` or
`gs://BUCKET2/prefix2/...`. The two are different real public/internal buckets with different directory
shapes (see "Why two test prefixes" below) — they are *not* the same bucket at two depths.

**Versions**: `gcloud` — Google Cloud SDK 569.0.0 (Python). `gcloud-rust` — this repo, both the
`cargo run` (dev/debug) binary and a `cargo build --release` binary were measured; see the call-out below
on why release numbers are the ones to trust for a fair comparison.

Method: bash's `time` builtin, `TIMEFORMAT='%R'` (wall-clock seconds only), 5 trials per scenario, run
back-to-back against the same live bucket. All commands' stdout was redirected to `/dev/null` (or piped
to `wc -l`, as noted) so terminal rendering isn't part of what's measured.

## 1. Reproducing the user's exact example

The original ask that prompted this doc — `time cargo run -- storage ls -r <uri> | wc -l` — reproduced
verbatim (bash, not zsh, so the `time` output shape differs slightly from a zsh session, but it's the
same wall-clock measurement):

```
$ time cargo run -- storage ls -r gs://BUCKET/prefix/ | wc -l
253
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.97s
     Running `target/debug/gcloud-rust storage ls -r 'gs://BUCKET/prefix/'`

real    0m1.996s
user    0m0.053s
sys     0m0.077s
```

**Caveat, flagged before drawing any conclusion from this one run**: `cargo run` includes Cargo's own
"is anything stale?" check (the `Finished ... in 0.97s` line) on top of the actual program run — that
check alone can be a large fraction of the measured 1.996s depending on disk cache state. It is *not* a
clean measurement of the binary's own speed. All comparative numbers below use the built binaries
directly (`target/debug/gcloud-rust` / `target/release/gcloud-rust`), invoked the same way `gcloud` is,
specifically to avoid this confound. If you only read one section, read the "Bulk recursive listing"
tables further down, not this one.

## 2. Why two test prefixes

One structural question has to be settled before any timing number means anything: **does `gcloud-rust`
return the same output as real `gcloud`, including at a prefix with real width and depth** — not just
the one prefix this project's formatting bugs were originally debugged against? So two prefixes were
used:

- **Prefix A**: a real-world internal bucket prefix, 3 levels deep (`date=.../v=.../file`), 21 immediate
  subdirectories, 253 lines of `ls -r` output including group headers.
- **Prefix B**: a public Google-sample bucket prefix, wider (12 immediate subdirectories, several with
  their own nested sub-structure), 375 lines of `ls -r` output.

Both were diffed **byte-for-byte**, not just line-counted:

```
$ gcloud storage ls -r gs://BUCKET2/prefix2/ > /tmp/gcloud_bq.txt
$ ./target/release/gcloud-rust storage ls -r gs://BUCKET2/prefix2/ > /tmp/rust_bq.txt
$ diff /tmp/gcloud_bq.txt /tmp/rust_bq.txt && echo IDENTICAL
IDENTICAL
```

(375 lines each, zero diff.) Same result for Prefix A (253 lines each, zero diff). So the timing
comparisons below are comparing identical output, not different amounts of work.

## 3. Startup latency only (`storage --help`, no network call)

Isolates interpreter/binary startup cost with no GCS API calls involved at all — this is the cleanest
read on the README's "fraction of the startup latency, no Python runtime" claim specifically.

| | min | mean | max |
|---|---|---|---|
| `gcloud storage --help` | 0.978s | 1.133s | 1.407s |
| `gcloud-rust storage --help` (release) | 0.006s | 0.007s | 0.008s |
| `gcloud-rust storage --help` (debug) | 0.007s | 0.007s | 0.008s* |

\* One debug trial measured 0.293s (first invocation after a fresh `cargo build`, likely binary not yet
in the OS page cache); the other 4 were 0.007–0.008s. Shown as a separate `(warm)` figure rather than
silently dropped: with that outlier included, debug's mean is 0.064s — still ~18x faster than `gcloud`,
just a less stable number than the warm-cache 0.007s.

**~140–160x faster**, comparing means. This gap is specifically the Python interpreter + SDK import cost
`gcloud` pays on every invocation regardless of what it's asked to do; a compiled binary doesn't have an
equivalent fixed cost.

## 4. Small single-request listing (`storage ls`, non-recursive, ~21 entries)

One GCS API call, small response — isolates "startup cost + one network round trip" from bulk data
volume.

| | min | mean | max |
|---|---|---|---|
| `gcloud storage ls` | 1.506s | 1.594s | 1.751s |
| `gcloud-rust storage ls` (release) | 0.465s | 0.504s | 0.547s |
| `gcloud-rust storage ls` (debug) | 0.500s | 0.509s | 0.535s |

**~3x faster.** Notably, debug and release are statistically indistinguishable here — with only one
network round trip, wall-clock time is dominated by the HTTP request + TLS handshake + auth, not by
compute, so compiler optimization level barely matters. The 3x gap that remains is still real and is
`gcloud`'s per-invocation Python overhead showing through even once network cost is added.

## 5. Bulk recursive listing (`storage ls -r`) — the main result

This is where the two tools' **request patterns diverge**, not just their language/runtime. Reading the
source (`src/storage/commands/ls.rs`): `gcloud-rust`'s recursive mode omits GCS's `delimiter` query
parameter entirely and asks for every object under the prefix in one flat, paginated listing (one HTTP
request per ~1000 objects). Real `gcloud storage ls -r`'s own timing behavior here is consistent with it
walking the prefix one directory at a time instead (one delimited list call per subdirectory level) —
**this is a difference in how much network round-tripping each approach needs for the same logical
request, not solely a Python-vs-Rust speed story.** Flagging this explicitly so the number below isn't
mistaken for a pure language benchmark.

**Prefix A** (253 lines, 21 subdirectories, 3 levels deep):

| | min | mean | max |
|---|---|---|---|
| `gcloud storage ls -r` | 6.715s | 6.816s | 6.922s |
| `gcloud-rust storage ls -r` (release) | 0.620s | 0.695s | 0.749s |
| `gcloud-rust storage ls -r` (debug) | 0.643s | 0.699s | 0.744s |

**~9.8x faster** (release mean vs. gcloud mean).

**Prefix B** (375 lines, 12 subdirectories with further nesting, wider than Prefix A):

| | min | mean | max |
|---|---|---|---|
| `gcloud storage ls -r` | 4.069s | 4.827s | 7.081s |
| `gcloud-rust storage ls -r` (release) | 0.526s | 0.566s | 0.628s |

**~8.5x faster** (release mean vs. gcloud mean). `gcloud`'s own run-to-run variance is much wider here
(4.07s–7.08s) than `gcloud-rust`'s (0.53s–0.63s) — consistent with "many sequential network round trips"
being more exposed to network jitter than "one or two paginated calls."

Debug vs. release for `gcloud-rust` are within noise of each other on both prefixes — same conclusion as
section 4: this workload is network-bound, not compute-bound, so the optimization level mostly doesn't
matter. The headline number to quote is the **release** figure either way.

## Summary

| Scenario | gcloud (mean) | gcloud-rust release (mean) | Speedup |
|---|---|---|---|
| `--help`, no network | 1.133s | 0.007s | ~160x |
| `ls`, 1 API call | 1.594s | 0.504s | ~3.2x |
| `ls -r`, Prefix A (253 lines) | 6.816s | 0.695s | ~9.8x |
| `ls -r`, Prefix B (375 lines) | 4.827s | 0.566s | ~8.5x |

Two distinct effects are stacked in the `ls -r` numbers: (1) `gcloud`'s fixed ~1s Python startup cost
that `gcloud-rust` doesn't pay, same as section 3, plus (2) a request-pattern difference (flat paginated
listing vs. apparent per-directory walking) that's specific to how each tool implements recursive listing
and isn't guaranteed to hold the same way for every other `gcloud` command this project might implement
in the future. Don't generalize the ~9x figure to commands that haven't been built yet.

## Caveats / what this doc does not claim

- Single machine, single network path, one afternoon. No repeated measurement across days/networks to
  check for diurnal network variance.
- No comparison of `cp`/`rm`/`cat` throughput for large objects (bandwidth-bound work, where both tools
  should converge since both ultimately move bytes over the same HTTPS connection) — only `ls`/`ls -r`
  were measured.
- `gcloud`'s recursive-listing request pattern was inferred from its timing behavior and general
  knowledge of how `gsutil`-lineage tools implement recursive listing, not from packet-level inspection
  (e.g. no `tcpdump`/proxy capture was done to literally count requests). If that inference is wrong, the
  "different request pattern" explanation in section 5 would need revisiting — the timing numbers
  themselves are real regardless.

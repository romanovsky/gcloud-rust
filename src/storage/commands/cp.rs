use std::path::{Path, PathBuf};

use crate::storage::commands::ls;
use crate::storage::{auth, http, uri};

pub(crate) fn run(src: &str, dst: &str, recursive: bool) -> crate::Result<()> {
    match (src.starts_with("gs://"), dst.starts_with("gs://")) {
        (false, false) => Err("storage cp: at least one of SRC/DST must be a gs:// URI \
            (use the regular `cp` for local-to-local copies)"
            .into()),
        (false, true) => upload(src, dst, recursive),
        (true, false) => download(src, dst, recursive),
        (true, true) => server_side_copy(src, dst, recursive),
    }
}

// ---------- upload (local -> gs://) ----------

fn upload(local_src: &str, gs_dst: &str, recursive: bool) -> crate::Result<()> {
    let path = Path::new(local_src);

    if path.is_dir() {
        if !recursive {
            return Err(format!(
                "storage cp: {local_src} is a directory; pass -r/--recursive to copy directories"
            )
            .into());
        }
        return upload_dir(path, gs_dst);
    }

    if !path.is_file() {
        return Err(format!("storage cp: no such local file: {local_src}").into());
    }

    upload_one(path, gs_dst)
}

fn upload_one(local_path: &Path, gs_dst: &str) -> crate::Result<()> {
    let dst = uri::parse(gs_dst)?;
    if dst.object.is_empty() {
        return Err("storage cp: destination must include an object name".into());
    }

    let token = auth::access_token()?;
    put_object(&token, &dst.bucket, &dst.object, local_path)?;
    println!(
        "Uploaded {} to gs://{}/{}",
        local_path.display(),
        dst.bucket,
        dst.object
    );
    Ok(())
}

/// UNVERIFIED LIVE: no disposable bucket available to upload a real directory
/// tree to in this environment. Nesting/rename math is unit-tested below.
fn upload_dir(local_root: &Path, gs_dst: &str) -> crate::Result<()> {
    let dst = uri::parse(gs_dst)?;
    let token = auth::access_token()?;

    let files = walk_files(local_root)?;
    let src_basename = local_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or("storage cp: source directory has no name")?;

    let dst_is_container = dst.object.is_empty() || gs_dst.ends_with('/');
    let dst_prefix = effective_destination(&dst.object, &src_basename, dst_is_container);

    let total = files.len();
    let mut failures = Vec::new();
    for (local_path, rel) in &files {
        let object_name = join_path(&dst_prefix, rel);
        if let Err(e) = put_object(&token, &dst.bucket, &object_name, local_path) {
            failures.push(format!("{}: {e}", local_path.display()));
        }
    }

    report("Uploaded", total, &failures)
}

fn put_object(token: &str, bucket: &str, object: &str, local_path: &Path) -> crate::Result<()> {
    let encoded = uri::encode_query_value(object);
    let url = format!(
        "https://storage.googleapis.com/upload/storage/v1/b/{bucket}/o?uploadType=media&name={encoded}"
    );
    http::upload_file(token, &url, local_path)?;
    Ok(())
}

// ---------- download (gs:// -> local) ----------

fn download(gs_src: &str, local_dst: &str, recursive: bool) -> crate::Result<()> {
    let src = uri::parse(gs_src)?;
    if src.object.is_empty() && !recursive {
        return Err(
            "storage cp: source must include an object name (pass --recursive to copy a whole bucket)"
                .into(),
        );
    }

    let token = auth::access_token()?;

    if !recursive {
        return download_one(&token, &src, local_dst);
    }

    // UNVERIFIED LIVE beyond the single-object fallback path (which reuses
    // the already-smoke-tested `download_one`): no disposable bucket to
    // exercise the multi-file batch loop against. Path-traversal guard and
    // nesting math are unit-tested below.
    let raw_prefix = src.object.trim_end_matches('/');
    let listing_prefix = if raw_prefix.is_empty() {
        String::new()
    } else {
        format!("{raw_prefix}/")
    };

    let names = ls::list_entries(&token, &src.bucket, &listing_prefix, true)?;
    if names.is_empty() {
        if src.object.is_empty() {
            return Err(format!(
                "storage cp: bucket gs://{} is empty, nothing to download",
                src.bucket
            )
            .into());
        }
        // Not a directory-like prefix -- maybe it's an exact object name.
        return download_one(&token, &src, local_dst);
    }

    let src_basename = basename_or(raw_prefix, &src.bucket);
    let dst_is_container = Path::new(local_dst).is_dir() || local_dst.ends_with('/');
    let dst_root = effective_destination(local_dst, &src_basename, dst_is_container);

    let total = names.len();
    let mut failures = Vec::new();
    for name in &names {
        let rel = name.strip_prefix(&listing_prefix).unwrap_or(name);
        if !is_safe_relative_path(rel) {
            failures.push(format!("{name}: unsafe relative path, skipped"));
            continue;
        }
        let dest_path = Path::new(&dst_root).join(rel);
        if let Err(e) = fetch_object(&token, &src.bucket, name, &dest_path) {
            failures.push(format!("{name}: {e}"));
        }
    }

    report("Downloaded", total, &failures)
}

fn download_one(token: &str, src: &uri::GsUri, local_dst: &str) -> crate::Result<()> {
    let dst_path = resolve_download_path(local_dst, &src.object);
    fetch_object(token, &src.bucket, &src.object, &dst_path)?;
    println!(
        "Downloaded gs://{}/{} to {}",
        src.bucket,
        src.object,
        dst_path.display()
    );
    Ok(())
}

fn fetch_object(token: &str, bucket: &str, object: &str, dest_path: &Path) -> crate::Result<()> {
    let encoded = uri::encode_object_name(object);
    let url =
        format!("https://storage.googleapis.com/storage/v1/b/{bucket}/o/{encoded}?alt=media");
    let body = http::get(token, &url)?;
    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(dest_path, &body)?;
    Ok(())
}

/// Mirrors `gcloud storage cp` behavior: an existing directory as DST means
/// "download into this directory using the object's basename".
fn resolve_download_path(dst: &str, object_name: &str) -> PathBuf {
    let dst_path = Path::new(dst);
    if dst_path.is_dir() {
        let basename = object_name.rsplit('/').next().unwrap_or(object_name);
        dst_path.join(basename)
    } else {
        dst_path.to_path_buf()
    }
}

// ---------- gs:// -> gs:// (server-side copy) ----------

fn server_side_copy(gs_src: &str, gs_dst: &str, recursive: bool) -> crate::Result<()> {
    let src = uri::parse(gs_src)?;
    let dst = uri::parse(gs_dst)?;
    let token = auth::access_token()?;

    if !recursive {
        if src.object.is_empty() || dst.object.is_empty() {
            return Err(
                "storage cp: both source and destination need an object name \
                 (pass --recursive for bucket-to-bucket copies)"
                    .into(),
            );
        }
        copy_one(&token, &src.bucket, &src.object, &dst.bucket, &dst.object)?;
        println!(
            "Copied gs://{}/{} to gs://{}/{}",
            src.bucket, src.object, dst.bucket, dst.object
        );
        return Ok(());
    }

    // UNVERIFIED LIVE: no second disposable bucket available to exercise
    // either the single-object fallback or the batch loop in this environment.
    let raw_prefix = src.object.trim_end_matches('/');
    let listing_prefix = if raw_prefix.is_empty() {
        String::new()
    } else {
        format!("{raw_prefix}/")
    };

    let names = ls::list_entries(&token, &src.bucket, &listing_prefix, true)?;
    if names.is_empty() {
        if src.object.is_empty() || dst.object.is_empty() {
            return Err(format!(
                "storage cp: nothing found under gs://{}/{}",
                src.bucket, raw_prefix
            )
            .into());
        }
        // Not a directory-like prefix -- maybe it's an exact object name.
        copy_one(&token, &src.bucket, &src.object, &dst.bucket, &dst.object)?;
        println!(
            "Copied gs://{}/{} to gs://{}/{}",
            src.bucket, src.object, dst.bucket, dst.object
        );
        return Ok(());
    }

    let src_basename = basename_or(raw_prefix, &src.bucket);
    let dst_is_container = dst.object.is_empty() || gs_dst.ends_with('/');
    let dst_prefix = effective_destination(&dst.object, &src_basename, dst_is_container);

    let total = names.len();
    let mut failures = Vec::new();
    for name in &names {
        let rel = name.strip_prefix(&listing_prefix).unwrap_or(name);
        let dst_object = join_path(&dst_prefix, rel);
        if let Err(e) = copy_one(&token, &src.bucket, name, &dst.bucket, &dst_object) {
            failures.push(format!("{name}: {e}"));
        }
    }

    report("Copied", total, &failures)
}

fn copy_one(
    token: &str,
    src_bucket: &str,
    src_object: &str,
    dst_bucket: &str,
    dst_object: &str,
) -> crate::Result<()> {
    let src_encoded = uri::encode_object_name(src_object);
    let dst_encoded = uri::encode_object_name(dst_object);
    let url = format!(
        "https://storage.googleapis.com/storage/v1/b/{src_bucket}/o/{src_encoded}/copyTo/b/{dst_bucket}/o/{dst_encoded}"
    );
    http::post_empty(token, &url)?;
    Ok(())
}

// ---------- shared helpers ----------

/// Walks `root` recursively, returning `(absolute_path, forward_slash_relative_path)`
/// for every regular file. Symlinks are skipped (not followed) -- matching
/// neither gcloud's `--ignore-symlinks` nor `--preserve-symlinks`; that
/// distinction is out of scope for v1.
fn walk_files(root: &Path) -> crate::Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    walk_files_into(root, root, &mut out)?;
    Ok(out)
}

fn walk_files_into(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(PathBuf, String)>,
) -> crate::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();

        if file_type.is_symlink() {
            continue;
        } else if file_type.is_dir() {
            walk_files_into(root, &path, out)?;
        } else if file_type.is_file() {
            let rel_path = path
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| path.clone());
            let rel_str = rel_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((path.clone(), rel_str));
        }
    }
    Ok(())
}

/// Rejects empty/`.`/`..` path segments so an unusual object name can't write
/// outside the intended download directory.
fn is_safe_relative_path(p: &str) -> bool {
    !p.split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
}

fn basename_or(prefix: &str, fallback: &str) -> String {
    match prefix.rsplit('/').next() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => fallback.to_string(),
    }
}

/// Implements gcloud storage's recursive-copy destination rule (verified
/// against `copy_task_iterator.py::_get_destination_suffix_for_recursion` in
/// the Google Cloud SDK source): nest the source's own basename under the
/// destination when the destination is/will-be a container; otherwise the
/// destination IS the new top-level name (rename semantics), matching Unix
/// `cp -r src dst`.
///
/// Simplification vs. real gcloud: for gs:// destinations we can't cheaply
/// probe whether the prefix already has objects under it (that would cost an
/// extra API call per invocation), so "is a container" is decided purely by
/// a trailing `/` on the literal DST string. Local destinations instead get
/// a real `Path::is_dir()` check from the caller, matching gcloud exactly.
fn effective_destination(dst: &str, src_basename: &str, dst_is_container: bool) -> String {
    if dst_is_container {
        join_path(dst, src_basename)
    } else {
        dst.trim_end_matches('/').to_string()
    }
}

fn join_path(base: &str, leaf: &str) -> String {
    if base.is_empty() {
        leaf.to_string()
    } else if base.ends_with('/') {
        format!("{base}{leaf}")
    } else {
        format!("{base}/{leaf}")
    }
}

fn report(verb: &str, total: usize, failures: &[String]) -> crate::Result<()> {
    let ok = total - failures.len();
    println!("{verb} {ok}/{total}");
    if failures.is_empty() {
        return Ok(());
    }
    for f in failures {
        eprintln!("  failed: {f}");
    }
    Err(format!(
        "{} of {} {} operations failed",
        failures.len(),
        total,
        verb.to_lowercase()
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nests_under_existing_container() {
        assert_eq!(
            effective_destination("existing_prefix", "base_dir", true),
            "existing_prefix/base_dir"
        );
    }

    #[test]
    fn renames_when_destination_does_not_exist() {
        assert_eq!(
            effective_destination("new_name", "base_dir", false),
            "new_name"
        );
    }

    #[test]
    fn container_destination_with_trailing_slash() {
        assert_eq!(
            effective_destination("new_prefix/", "base_dir", true),
            "new_prefix/base_dir"
        );
    }

    #[test]
    fn empty_container_destination_is_just_basename() {
        assert_eq!(effective_destination("", "base_dir", true), "base_dir");
    }

    #[test]
    fn join_path_handles_trailing_slash() {
        assert_eq!(join_path("a/", "b"), "a/b");
        assert_eq!(join_path("a", "b"), "a/b");
        assert_eq!(join_path("", "b"), "b");
    }

    #[test]
    fn basename_or_falls_back_when_empty() {
        assert_eq!(basename_or("", "my-bucket"), "my-bucket");
        assert_eq!(basename_or("dir/sub", "my-bucket"), "sub");
    }

    #[test]
    fn rejects_unsafe_relative_paths() {
        assert!(!is_safe_relative_path("../etc/passwd"));
        assert!(!is_safe_relative_path("a/../b"));
        assert!(!is_safe_relative_path("a//b"));
        assert!(is_safe_relative_path("a/b/c.txt"));
    }
}

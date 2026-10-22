use anyhow::Context;

use crate::storage::{auth, http, sanitize, uri};

pub(crate) fn run(uri_str: &str, recursive: bool) -> crate::Result<()> {
    let gs = uri::parse(uri_str)?;
    let token = auth::access_token()?;
    let entries = list_entries(&token, &gs.bucket, &gs.object, recursive)?;

    if recursive {
        print!("{}", format_recursive_tree(&gs.bucket, &gs.object, &entries));
    } else {
        for name in &entries {
            if !name.is_empty() {
                println!("gs://{}/{}", gs.bucket, sanitize(name));
            }
        }
    }

    Ok(())
}

/// Builds the grouped, headered listing text matching `gcloud storage ls -r`
/// output format. Returns a `String` (rather than printing directly) so the
/// formatting rules can be unit-tested without capturing stdout. Example
/// shape (illustrative text, not a doctest):
///
/// ```text
/// gs://bucket/prefix/:
///
/// gs://bucket/prefix/subdir/:
/// gs://bucket/prefix/subdir/file1
/// gs://bucket/prefix/subdir/file2
/// ```
///
/// Two quirks matched against real `gcloud storage ls -r` output, confirmed
/// empirically against `gs://cloud-samples-data/` (bucket root) and
/// `gs://cloud-samples-data/bigquery/` (a prefix with a placeholder object):
///
/// - The prefix itself (even the empty bucket-root prefix) always gets its
///   own `:`-suffixed header group -- there is no special case for "root".
/// - If a placeholder/folder-marker object exists whose name is *exactly*
///   the queried prefix, gcloud prints it once more as a bare line (no
///   trailing `:`) plus a blank line, before any group headers at all.
fn format_recursive_tree(bucket: &str, prefix: &str, entries: &[String]) -> String {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    let mut out = String::new();

    // Bare top-line quirk: only when a literal placeholder object matches
    // the queried prefix exactly.
    if !prefix.is_empty() && entries.iter().any(|e| e == prefix) {
        let _ = writeln!(out, "gs://{}/{}", bucket, sanitize(prefix));
        let _ = writeln!(out);
    }

    let mut dirs = BTreeSet::new();
    // The query prefix itself is always a group header, including the empty
    // (bucket-root) prefix.
    dirs.insert(prefix.to_string());

    for name in entries {
        // Parent dir = everything up to and including the last '/'.
        if let Some(slash) = name.rfind('/') {
            let parent = &name[..=slash];
            dirs.insert(parent.to_string());

            // Insert intermediate dirs between the query prefix and the parent.
            if let Some(rest) = parent.strip_prefix(prefix) {
                let mut current = prefix.to_string();
                for segment in rest.split('/') {
                    if segment.is_empty() {
                        continue;
                    }
                    current.push_str(segment);
                    current.push('/');
                    dirs.insert(current.clone());
                }
            }
        }
    }

    let dir_list: Vec<_> = dirs.into_iter().collect();
    for (i, dir) in dir_list.iter().enumerate() {
        let _ = writeln!(out, "gs://{}/{}:", bucket, sanitize(dir));

        // Objects whose immediate parent is this directory.
        for name in entries {
            let parent = match name.rfind('/') {
                Some(i) => &name[..=i],
                None => "",
            };
            if parent == dir.as_str() {
                let _ = writeln!(out, "gs://{}/{}", bucket, sanitize(name));
            }
        }

        // Blank separator between groups, but not after the last one.
        if i + 1 < dir_list.len() {
            let _ = writeln!(out);
        }
    }

    out
}

/// Lists object (and, when non-recursive, common-prefix) names directly under
/// `prefix` in `bucket`, following `nextPageToken` until exhausted. When
/// `recursive` is true, `delimiter` is omitted, so GCS returns every object
/// under the prefix in one flat (but still paginated) list. Shared with
/// `rm -r` and `cp -r` for recursive enumeration.
pub(crate) fn list_entries(
    token: &str,
    bucket: &str,
    prefix: &str,
    recursive: bool,
) -> crate::Result<Vec<String>> {
    let encoded_prefix = uri::encode_query_value(prefix);
    let delimiter_param = if recursive { "" } else { "delimiter=%2F&" };

    let mut entries = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut url = format!(
            "https://storage.googleapis.com/storage/v1/b/{bucket}/o?{delimiter_param}prefix={encoded_prefix}"
        );
        if let Some(token_value) = &page_token {
            url.push_str("&pageToken=");
            url.push_str(&uri::encode_query_value(token_value));
        }

        let body = http::get(token, &url)?;
        let resp: GcsListResponse =
            serde_json::from_slice(&body).context("failed to parse GCS list response")?;

        if let Some(prefixes) = resp.prefixes {
            entries.extend(prefixes);
        }
        if let Some(items) = resp.items {
            entries.extend(items.into_iter().map(|item| item.name));
        }

        match resp.next_page_token {
            Some(next) if !next.is_empty() => page_token = Some(next),
            _ => break,
        }
    }

    Ok(entries)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GcsListResponse {
    prefixes: Option<Vec<String>>,
    items: Option<Vec<GcsObject>>,
    next_page_token: Option<String>,
}

#[derive(serde::Deserialize)]
struct GcsObject {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bucket_root_gets_its_own_header() {
        // Regression test: a literal `gs://bucket/:` header must appear even
        // for an empty prefix (whole-bucket recursive listing) -- confirmed
        // against real `gcloud storage ls -r gs://cloud-samples-data/`.
        let out = format_recursive_tree("b", "", &entries(&["top.txt", "dir/nested.txt"]));
        assert!(
            out.starts_with("gs://b/:\n"),
            "expected root header first, got:\n{out}"
        );
    }

    #[test]
    fn placeholder_object_gets_bare_top_line() {
        // Regression test: when an object's name exactly equals the queried
        // prefix (a GCS folder-marker placeholder), gcloud prints it once as
        // a bare line + blank line before any `:` headers -- confirmed
        // against gs://cloud-samples-data/bigquery/.
        let out = format_recursive_tree("b", "dir/", &entries(&["dir/", "dir/file.txt"]));
        let mut lines = out.lines();
        assert_eq!(lines.next(), Some("gs://b/dir/"));
        assert_eq!(lines.next(), Some(""));
        assert_eq!(lines.next(), Some("gs://b/dir/:"));
    }

    #[test]
    fn no_placeholder_means_no_bare_top_line() {
        // Regression guard: buckets with no folder-marker object (the common
        // case, e.g. the adtech bucket this was first verified against) must
        // NOT gain a spurious bare top-line.
        let out = format_recursive_tree("b", "dir/", &entries(&["dir/file.txt"]));
        assert!(out.starts_with("gs://b/dir/:\n"), "got:\n{out}");
    }

    #[test]
    fn last_group_has_no_trailing_blank_line() {
        let out = format_recursive_tree("b", "dir/", &entries(&["dir/file.txt"]));
        assert!(!out.ends_with("\n\n"), "got:\n{out:?}");
    }

    #[test]
    fn object_names_are_sanitized_before_printing() {
        let out = format_recursive_tree("b", "dir/", &entries(&["dir/\x1b[31mfile.txt"]));
        assert!(!out.contains('\x1b'), "escape sequence leaked into:\n{out:?}");
    }
}

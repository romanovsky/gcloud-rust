use crate::storage::{auth, http, uri};

pub(crate) fn run(uri_str: &str, recursive: bool) -> crate::Result<()> {
    let gs = uri::parse(uri_str)?;
    let token = auth::access_token()?;
    let entries = list_entries(&token, &gs.bucket, &gs.object, recursive)?;

    if recursive {
        print_recursive_tree(&gs.bucket, &gs.object, &entries);
    } else {
        for name in &entries {
            if !name.is_empty() {
                println!("gs://{}/{}", gs.bucket, name);
            }
        }
    }

    Ok(())
}

/// Prints objects grouped by parent directory with headers, matching
/// `gcloud storage ls -r` output format:
///
///     gs://bucket/prefix/:
///
///     gs://bucket/prefix/subdir/:
///     gs://bucket/prefix/subdir/file1
///     gs://bucket/prefix/subdir/file2
///
fn print_recursive_tree(bucket: &str, prefix: &str, entries: &[String]) {
    use std::collections::BTreeSet;

    let mut dirs = BTreeSet::new();
    // The query prefix itself is always a group header.
    if !prefix.is_empty() {
        dirs.insert(prefix.to_string());
    }

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
        println!("gs://{}/{}:", bucket, dir);

        // Objects whose immediate parent is this directory.
        for name in entries {
            let parent = match name.rfind('/') {
                Some(i) => &name[..=i],
                None => "",
            };
            if parent == dir.as_str() {
                println!("gs://{}/{}", bucket, name);
            }
        }

        // Blank separator between groups, but not after the last one.
        if i + 1 < dir_list.len() {
            println!();
        }
    }
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
        let resp: GcsListResponse = serde_json::from_slice(&body)
            .map_err(|e| format!("failed to parse GCS response: {e}"))?;

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

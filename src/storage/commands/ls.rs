use std::io::Write;
use std::process::{Command, Stdio};

use crate::storage::{auth, http, uri};

pub(crate) fn run(uri_str: &str, recursive: bool) -> crate::Result<()> {
    let gs = uri::parse(uri_str)?;
    let token = auth::access_token()?;

    for name in list_entries(&token, &gs.bucket, &gs.object, recursive)? {
        if !name.is_empty() {
            println!("gs://{}/{}", gs.bucket, name);
        }
    }

    Ok(())
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
        let json = String::from_utf8(body).map_err(|_| "GCS returned a non-UTF8 response")?;

        entries.extend(run_jq(&json, "(.prefixes // [])[], (.items // [])[].name")?);

        match run_jq(&json, "(.nextPageToken // empty)")?.into_iter().next() {
            Some(next) if !next.is_empty() => page_token = Some(next),
            _ => break,
        }
    }

    Ok(entries)
}

/// Runs `jq -r <filter>` over `json`, returning one `String` per output line.
fn run_jq(json: &str, filter: &str) -> crate::Result<Vec<String>> {
    let mut child = Command::new("jq")
        .arg("-r")
        .arg(filter)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run `jq` ({e}). Is it installed and on PATH?"))?;

    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(json.as_bytes())?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "jq failed to parse the GCS response: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

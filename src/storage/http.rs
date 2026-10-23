use std::io::Read as _;
use std::path::Path;

use anyhow::{Context, bail};

/// Native HTTP client for the GCS JSON API, using `ureq`.
///
/// `http_status_as_error(false)` is set on every request: ureq 3.x's default
/// behavior collapses any 4xx/5xx into `Error::StatusCode(u16)`, which drops
/// the response body entirely. GCS's JSON error bodies
/// (`{"error": {"message": "..."}}`) are far more useful than a bare status
/// code, so we disable that and read+report the body ourselves -- restoring
/// the behavior the old curl implementation got from `--fail-with-body`.
fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// Reads the full response body and, for non-2xx statuses, turns it into an
/// error carrying the (sanitized) body text instead of just the status code.
fn finish(resp: http::Response<ureq::Body>, verb: &str, url: &str) -> crate::Result<Vec<u8>> {
    let status = resp.status();

    let mut body = Vec::new();
    resp.into_body()
        .into_reader()
        .read_to_end(&mut body)
        .with_context(|| format!("{verb} {url}: failed to read response body"))?;

    if !status.is_success() {
        let text = crate::storage::sanitize(String::from_utf8_lossy(&body).trim());
        bail!("{verb} {url} failed: HTTP {} - {text}", status.as_u16());
    }

    Ok(body)
}

pub(crate) fn get(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = ureq::get(url)
        .header("Authorization", &bearer(token))
        .config()
        .http_status_as_error(false)
        .build()
        .call()
        .with_context(|| format!("GET {url} failed"))?;
    finish(resp, "GET", url)
}

pub(crate) fn delete(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = ureq::delete(url)
        .header("Authorization", &bearer(token))
        .config()
        .http_status_as_error(false)
        .build()
        .call()
        .with_context(|| format!("DELETE {url} failed"))?;
    finish(resp, "DELETE", url)
}

pub(crate) fn post_empty(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = ureq::post(url)
        .header("Authorization", &bearer(token))
        .header("Content-Length", "0")
        .config()
        .http_status_as_error(false)
        .build()
        .send(&b""[..])
        .with_context(|| format!("POST {url} failed"))?;
    finish(resp, "POST", url)
}

/// Buffers the whole file into the request (simple media upload, not resumable).
pub(crate) fn upload_file(token: &str, url: &str, local_path: &Path) -> crate::Result<Vec<u8>> {
    let file = std::fs::File::open(local_path)
        .with_context(|| format!("opening {}", local_path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("reading metadata for {}", local_path.display()))?
        .len();

    let resp = ureq::post(url)
        .header("Authorization", &bearer(token))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", &len.to_string())
        .config()
        .http_status_as_error(false)
        .build()
        .send(file)
        .with_context(|| format!("upload {url} failed"))?;
    finish(resp, "upload", url)
}

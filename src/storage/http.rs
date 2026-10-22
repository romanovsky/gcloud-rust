use std::io::Read as _;
use std::path::Path;

/// Native HTTP client using `ureq`. Replaces the previous shell-out to `curl`
/// now that crates.io dependencies route through the Walmart Artifactory proxy.

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn read_body(resp: ureq::Body) -> crate::Result<Vec<u8>> {
    let mut body = Vec::new();
    resp.into_reader().read_to_end(&mut body)?;
    Ok(body)
}

pub(crate) fn get(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = ureq::get(url)
        .header("Authorization", &bearer(token))
        .call()
        .map_err(|e| format!("GET {url} failed: {e}"))?;
    read_body(resp.into_body())
}

pub(crate) fn delete(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = ureq::delete(url)
        .header("Authorization", &bearer(token))
        .call()
        .map_err(|e| format!("DELETE {url} failed: {e}"))?;
    read_body(resp.into_body())
}

pub(crate) fn post_empty(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = ureq::post(url)
        .header("Authorization", &bearer(token))
        .header("Content-Length", "0")
        .send(&b""[..])
        .map_err(|e| format!("POST {url} failed: {e}"))?;
    read_body(resp.into_body())
}

/// Buffers the whole file into the request (simple media upload, not resumable).
pub(crate) fn upload_file(token: &str, url: &str, local_path: &Path) -> crate::Result<Vec<u8>> {
    let file = std::fs::File::open(local_path)?;
    let len = file.metadata()?.len();
    let resp = ureq::post(url)
        .header("Authorization", &bearer(token))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", &len.to_string())
        .send(file)
        .map_err(|e| format!("upload {url} failed: {e}"))?;
    read_body(resp.into_body())
}

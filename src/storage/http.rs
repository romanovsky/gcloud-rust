use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Thin wrapper around the system `curl` binary. We shell out instead of using
/// an HTTP crate (reqwest/hyper) because this environment's Cargo registry
/// downloads (static.crates.io) are blocked by the corporate proxy (403) —
/// confirmed via direct `curl` test, independent of Cargo. `curl` itself,
/// `gcloud`, and `jq` are plain system binaries, not crates.io dependencies.
struct CurlResponse {
    success: bool,
    body: Vec<u8>,
    stderr: String,
}

/// Runs curl with the bearer token injected via a `-K -` config file on stdin,
/// so the token never appears in argv — and therefore never leaks into `ps`
/// output / process listings visible to other local users.
fn run(token: &str, args: &[&str]) -> crate::Result<CurlResponse> {
    let config = format!(
        "header = \"Authorization: Bearer {}\"\n",
        token.replace('\\', "\\\\").replace('"', "\\\"")
    );

    let mut child = Command::new("curl")
        .arg("-sS")
        .arg("--fail-with-body")
        .arg("-K")
        .arg("-")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run `curl` ({e}). Is it installed and on PATH?"))?;

    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(config.as_bytes())?;

    let output = child.wait_with_output()?;

    Ok(CurlResponse {
        success: output.status.success(),
        body: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn ensure_ok(resp: CurlResponse, verb: &str, url: &str) -> crate::Result<Vec<u8>> {
    if resp.success {
        return Ok(resp.body);
    }
    let body_text = String::from_utf8_lossy(&resp.body);
    let detail = if body_text.trim().is_empty() {
        resp.stderr
    } else {
        body_text.trim().to_string()
    };
    Err(format!("{verb} {url} failed: {detail}").into())
}

pub(crate) fn get(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = run(token, &[url])?;
    ensure_ok(resp, "GET", url)
}

pub(crate) fn delete(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = run(token, &["-X", "DELETE", url])?;
    ensure_ok(resp, "DELETE", url)
}

pub(crate) fn post_empty(token: &str, url: &str) -> crate::Result<Vec<u8>> {
    let resp = run(token, &["-X", "POST", "-H", "Content-Length: 0", url])?;
    ensure_ok(resp, "POST", url)
}

/// Buffers the whole file into the request (simple media upload, not resumable).
/// Fine for a v1 skeleton; large-file resumable uploads are out of scope.
pub(crate) fn upload_file(token: &str, url: &str, local_path: &Path) -> crate::Result<Vec<u8>> {
    let path_str = local_path.to_str().ok_or("local path is not valid UTF-8")?;
    let data_arg = format!("@{path_str}");
    let resp = run(
        token,
        &[
            "-X",
            "POST",
            "-H",
            "Content-Type: application/octet-stream",
            "--data-binary",
            &data_arg,
            url,
        ],
    )?;
    ensure_ok(resp, "upload", url)
}

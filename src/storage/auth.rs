use std::process::Command;

/// Fetches a short-lived OAuth2 access token from the active `gcloud` session.
/// Shelling out to `gcloud` means we never touch service-account keys, refresh
/// tokens, or ADC file parsing ourselves — ordinary `gcloud auth login` is enough.
pub(crate) fn access_token() -> crate::Result<String> {
    let output = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .map_err(|e| {
            format!("failed to run `gcloud` ({e}). Is the Google Cloud SDK installed and on PATH?")
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`gcloud auth print-access-token` failed: {}\nRun `gcloud auth login` first.",
            stderr.trim()
        )
        .into());
    }

    let token = String::from_utf8(output.stdout)
        .map_err(|_| "gcloud returned a non-UTF8 access token")?
        .trim()
        .to_string();

    if token.is_empty() {
        return Err("gcloud returned an empty access token; run `gcloud auth login`".into());
    }

    Ok(token)
}

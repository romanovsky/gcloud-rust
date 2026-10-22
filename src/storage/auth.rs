use std::path::PathBuf;

use anyhow::{Context, bail};

/// Fetches a short-lived OAuth2 access token by reading the local ADC
/// (Application Default Credentials) file and exchanging the refresh token
/// with Google's OAuth2 endpoint directly. ADC is populated by
/// `gcloud auth application-default login` -- a *different* credential store
/// than the one `gcloud auth login` sets up for `gcloud auth
/// print-access-token`. If the ADC file is missing or malformed, we fall back
/// to shelling out to `gcloud auth print-access-token`, so either login flow
/// works.
pub(crate) fn access_token() -> crate::Result<String> {
    match token_from_adc() {
        Ok(token) => Ok(token),
        Err(_) => token_from_gcloud(),
    }
}

fn adc_path() -> Option<PathBuf> {
    // $GOOGLE_APPLICATION_CREDENTIALS takes precedence, then the standard location.
    if let Ok(p) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config/gcloud/application_default_credentials.json"))
}

#[derive(serde::Deserialize)]
struct AdcCredentials {
    client_id: String,
    client_secret: String,
    refresh_token: String,
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
}

fn token_from_adc() -> crate::Result<String> {
    let path = adc_path().context("could not determine ADC path")?;
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let creds: AdcCredentials =
        serde_json::from_str(&data).context("invalid ADC file")?;

    let resp = ureq::post("https://oauth2.googleapis.com/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .config()
        .http_status_as_error(false)
        .build()
        .send(
            format!(
                "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
                creds.client_id, creds.client_secret, creds.refresh_token
            )
            .as_bytes(),
        )
        .context("OAuth2 token refresh request failed")?;

    if !resp.status().is_success() {
        bail!("OAuth2 token refresh failed: HTTP {}", resp.status().as_u16());
    }

    let token_resp: TokenResponse = serde_json::from_reader(resp.into_body().into_reader())
        .context("failed to parse OAuth2 token response")?;

    if token_resp.access_token.is_empty() {
        bail!("OAuth2 returned an empty access token");
    }

    Ok(token_resp.access_token)
}

/// Fallback: shell out to gcloud CLI.
fn token_from_gcloud() -> crate::Result<String> {
    let output = std::process::Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .context("failed to run `gcloud`. Is the Google Cloud SDK installed and on PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "`gcloud auth print-access-token` failed: {}\nRun `gcloud auth login` first.",
            stderr.trim()
        );
    }

    let token = String::from_utf8(output.stdout)
        .context("gcloud returned a non-UTF8 access token")?
        .trim()
        .to_string();

    if token.is_empty() {
        bail!("gcloud returned an empty access token; run `gcloud auth login`");
    }

    Ok(token)
}

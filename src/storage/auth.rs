use std::path::PathBuf;

/// Fetches a short-lived OAuth2 access token by reading the local ADC
/// (Application Default Credentials) file and exchanging the refresh token
/// with Google's OAuth2 endpoint.  Falls back to `gcloud auth print-access-token`
/// if the ADC file is missing or malformed, so `gcloud auth login` still works
/// as the only prerequisite.
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
    let path = adc_path().ok_or("could not determine ADC path")?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let creds: AdcCredentials =
        serde_json::from_str(&data).map_err(|e| format!("invalid ADC file: {e}"))?;

    let resp = ureq::post("https://oauth2.googleapis.com/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(
            format!(
                "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
                creds.client_id, creds.client_secret, creds.refresh_token
            )
            .as_bytes(),
        )
        .map_err(|e| format!("OAuth2 token refresh failed: {e}"))?;

    let token_resp: TokenResponse = serde_json::from_reader(resp.into_body().into_reader())
        .map_err(|e| format!("failed to parse token response: {e}"))?;

    if token_resp.access_token.is_empty() {
        return Err("OAuth2 returned an empty access token".into());
    }

    Ok(token_resp.access_token)
}

/// Fallback: shell out to gcloud CLI.
fn token_from_gcloud() -> crate::Result<String> {
    let output = std::process::Command::new("gcloud")
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

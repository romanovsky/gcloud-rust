pub(crate) struct GsUri {
    pub bucket: String,
    /// Object name, or the empty string for a bucket-only URI (`gs://bucket`).
    pub object: String,
}

pub(crate) fn parse(s: &str) -> crate::Result<GsUri> {
    let rest = s
        .strip_prefix("gs://")
        .ok_or_else(|| anyhow::anyhow!("not a gs:// URI: `{s}`"))?;

    let (bucket, object) = match rest.split_once('/') {
        Some((b, o)) => (b.to_string(), o.to_string()),
        None => (rest.to_string(), String::new()),
    };

    if bucket.is_empty() {
        anyhow::bail!("missing bucket name in `{s}`");
    }

    Ok(GsUri { bucket, object })
}

/// Percent-encodes a GCS object name for use as a JSON API path segment
/// (`/b/{bucket}/o/{object}`), per RFC 3986 unreserved characters. Object
/// names are opaque and may contain `/`, so it is encoded too (as `%2F`).
pub(crate) fn encode_object_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for b in name.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Percent-encodes a query-string value. Same rules as object names suffice here.
pub(crate) fn encode_query_value(value: &str) -> String {
    encode_object_name(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bucket_and_object() {
        let gs = parse("gs://my-bucket/path/to/object.txt").unwrap();
        assert_eq!(gs.bucket, "my-bucket");
        assert_eq!(gs.object, "path/to/object.txt");
    }

    #[test]
    fn parses_bucket_only() {
        let gs = parse("gs://my-bucket").unwrap();
        assert_eq!(gs.bucket, "my-bucket");
        assert_eq!(gs.object, "");
    }

    #[test]
    fn rejects_non_gs_uri() {
        assert!(parse("s3://bucket/key").is_err());
    }

    #[test]
    fn rejects_missing_bucket() {
        assert!(parse("gs:///key").is_err());
    }

    #[test]
    fn encodes_special_characters() {
        assert_eq!(encode_object_name("a b/c"), "a%20b%2Fc");
    }
}

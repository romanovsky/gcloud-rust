use std::io::Write;

use crate::storage::{auth, http, uri};

pub(crate) fn run(uri_str: &str) -> crate::Result<()> {
    let gs = uri::parse(uri_str)?;
    if gs.object.is_empty() {
        return Err("storage cat: URI must include an object, e.g. gs://bucket/object".into());
    }

    let token = auth::access_token()?;
    let object = uri::encode_object_name(&gs.object);
    let url = format!(
        "https://storage.googleapis.com/storage/v1/b/{}/o/{}?alt=media",
        gs.bucket, object
    );

    // Buffered, not streamed: fine for a v1 skeleton, not for multi-GB objects.
    let body = http::get(&token, &url)?;
    std::io::stdout().write_all(&body)?;
    Ok(())
}

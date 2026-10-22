use crate::commands::ls;
use crate::{auth, http, uri};

pub(crate) fn run(uri_str: &str, recursive: bool) -> crate::Result<()> {
    let gs = uri::parse(uri_str)?;
    let token = auth::access_token()?;

    if !recursive {
        if gs.object.is_empty() {
            return Err(
                "storage rm: URI must include an object, e.g. gs://bucket/object \
                 (pass --recursive to delete a whole bucket)"
                    .into(),
            );
        }
        delete_one(&token, &gs.bucket, &gs.object)?;
        println!("Removed gs://{}/{}", gs.bucket, gs.object);
        return Ok(());
    }

    // UNVERIFIED LIVE: destructive, and this environment has no disposable
    // bucket to test against. The nesting/prefix logic is unit-tested, and
    // `delete_one`/`list_entries` reuse code paths exercised by `ls`/`rm`
    // (non-recursive), but the full recursive-delete path itself has not
    // been run against a real bucket.
    let raw_prefix = gs.object.trim_end_matches('/');
    let listing_prefix = if raw_prefix.is_empty() {
        String::new()
    } else {
        format!("{raw_prefix}/")
    };

    let mut names = ls::list_entries(&token, &gs.bucket, &listing_prefix, true)?;
    if names.is_empty() && !raw_prefix.is_empty() {
        // Might be an exact object name rather than a directory-like prefix.
        names.push(raw_prefix.to_string());
    }

    let total = names.len();
    let mut failures = Vec::new();
    for name in &names {
        if let Err(e) = delete_one(&token, &gs.bucket, name) {
            failures.push(format!("{name}: {e}"));
        }
    }

    let ok = total - failures.len();
    println!(
        "Removed {ok}/{total} object(s) under gs://{}/{}",
        gs.bucket, raw_prefix
    );

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("  failed: {f}");
        }
        return Err(format!("{} of {} deletions failed", failures.len(), total).into());
    }

    if gs.object.is_empty() {
        http::delete(
            &token,
            &format!("https://storage.googleapis.com/storage/v1/b/{}", gs.bucket),
        )?;
        println!("Removed bucket gs://{}", gs.bucket);
    }

    Ok(())
}

fn delete_one(token: &str, bucket: &str, object: &str) -> crate::Result<()> {
    let encoded = uri::encode_object_name(object);
    let url = format!("https://storage.googleapis.com/storage/v1/b/{bucket}/o/{encoded}");
    http::delete(token, &url)?;
    Ok(())
}

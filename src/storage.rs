use axum::body::Body;
use std::{
    fs::{create_dir_all, File},
    io::Write,
};

pub(crate) fn sanitize_string(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '/' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) async fn write_blob(org: &str, repo: &str, req_digest_string: &str, body: Body) -> bool {
    let bytes_res = axum::body::to_bytes(body, usize::MAX).await;
    if bytes_res.is_err() {
        return false;
    }
    let bytes = bytes_res.unwrap();

    let req_digest = req_digest_string
        .strip_prefix("sha256:")
        .unwrap_or(req_digest_string);
    let body_digest = sha256::digest(bytes.as_ref());
    let matches = req_digest == body_digest;

    log::info!(
        "storage/write_file: digest: {}, body_digest: {}, matches: {}",
        req_digest,
        body_digest,
        matches
    );

    if !matches {
        return false;
    }

    let base_path = format!(
        "./tmp/blobs/{}/{}",
        sanitize_string(org),
        sanitize_string(repo),
    );

    write_bytes_to_file(&base_path, req_digest, &bytes).await
}

pub(crate) async fn write_manifest(org: &str, repo: &str, reference: &str, body: Body) -> bool {
    let bytes_res = axum::body::to_bytes(body, usize::MAX).await;
    if bytes_res.is_err() {
        return false;
    }
    let bytes = bytes_res.unwrap();

    let base_path = format!(
        "./tmp/manifests/{}/{}",
        sanitize_string(org),
        sanitize_string(repo),
    );

    write_bytes_to_file(&base_path, reference, &bytes).await
}

pub(crate) async fn write_bytes_to_file(base_path: &str, file_name: &str, bytes: &[u8]) -> bool {
    if let Err(e) = create_dir_all(base_path) {
        log::error!("storage/write_file: error creating directory: {}", e);
        return false;
    }

    let mut file = match File::create(format!("{}/{}", base_path, file_name)) {
        Ok(file) => file,
        Err(e) => {
            log::error!("storage/write_file: error creating file: {}", e);
            return false;
        }
    };

    if let Err(e) = file.write_all(bytes) {
        log::error!("storage/write_file: error writing to file: {}", e);
        return false;
    }

    if let Err(e) = file.flush() {
        log::error!("storage/write_file: error flushing file: {}", e);
        return false;
    }

    log::info!("storage/write_file: wrote to {}", base_path);

    true
}

pub(crate) fn read_blob(org: &str, repo: &str, digest: &str) -> Result<Vec<u8>, std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_digest = sanitize_string(digest);

    let blob_path = format!(
        "./tmp/blobs/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_digest
    );
    std::fs::read(blob_path)
}

pub(crate) fn blob_metadata(
    org: &str,
    repo: &str,
    digest: &str,
) -> Result<std::fs::Metadata, std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_digest = sanitize_string(digest);

    let blob_path = format!(
        "./tmp/blobs/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_digest
    );
    std::fs::metadata(blob_path)
}

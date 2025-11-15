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

pub(crate) async fn write_manifest_bytes(
    org: &str,
    repo: &str,
    reference: &str,
    bytes: &[u8],
) -> bool {
    let base_path = format!(
        "./tmp/manifests/{}/{}",
        sanitize_string(org),
        sanitize_string(repo),
    );

    write_bytes_to_file(&base_path, reference, bytes).await
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

pub(crate) fn read_manifest(
    org: &str,
    repo: &str,
    reference: &str,
) -> Result<Vec<u8>, std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_reference = sanitize_string(reference);

    let manifest_path = format!(
        "./tmp/manifests/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_reference
    );
    std::fs::read(manifest_path)
}

pub(crate) fn manifest_exists(org: &str, repo: &str, reference: &str) -> bool {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_reference = sanitize_string(reference);

    let manifest_path = format!(
        "./tmp/manifests/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_reference
    );
    std::path::Path::new(&manifest_path).exists()
}

pub(crate) fn list_tags(org: &str, repo: &str) -> Result<Vec<String>, std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);

    let manifests_dir = format!("./tmp/manifests/{}/{}", sanitized_org, sanitized_repo);
    let path = std::path::Path::new(&manifests_dir);

    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut tags = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.path().is_file() {
            if let Some(filename) = entry.file_name().to_str() {
                // Filter out digest references (start with sha256:)
                // Only include tag names
                if !filename.starts_with("sha256:") {
                    tags.push(filename.to_string());
                }
            }
        }
    }

    // Sort tags alphabetically for consistent ordering
    tags.sort();
    Ok(tags)
}

pub(crate) fn init_upload_session(org: &str, repo: &str, uuid: &str) -> Result<(), std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_uuid = sanitize_string(uuid);

    let upload_dir = format!("./tmp/uploads/{}/{}", sanitized_org, sanitized_repo);
    std::fs::create_dir_all(&upload_dir)?;

    let upload_path = format!("{}/{}", upload_dir, sanitized_uuid);
    std::fs::File::create(upload_path)?;
    Ok(())
}

pub(crate) fn append_upload_chunk(
    org: &str,
    repo: &str,
    uuid: &str,
    chunk_data: &[u8],
) -> Result<u64, std::io::Error> {
    use std::fs::OpenOptions;

    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_uuid = sanitize_string(uuid);

    let upload_path = format!(
        "./tmp/uploads/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_uuid
    );

    let mut file = OpenOptions::new().append(true).open(&upload_path)?;

    file.write_all(chunk_data)?;

    let metadata = std::fs::metadata(&upload_path)?;
    Ok(metadata.len())
}

pub(crate) fn finalize_upload(
    org: &str,
    repo: &str,
    uuid: &str,
    expected_digest: &str,
) -> Result<String, String> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_uuid = sanitize_string(uuid);

    let upload_path = format!(
        "./tmp/uploads/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_uuid
    );

    let upload_data =
        std::fs::read(&upload_path).map_err(|e| format!("Failed to read upload: {}", e))?;

    let actual_digest = sha256::digest(&upload_data);
    let clean_expected = expected_digest
        .strip_prefix("sha256:")
        .unwrap_or(expected_digest);

    if actual_digest != clean_expected {
        return Err(format!(
            "Digest mismatch: expected {}, got {}",
            clean_expected, actual_digest
        ));
    }

    let blob_dir = format!("./tmp/blobs/{}/{}", sanitized_org, sanitized_repo);
    std::fs::create_dir_all(&blob_dir).map_err(|e| format!("Failed to create blob dir: {}", e))?;

    let blob_path = format!("{}/{}", blob_dir, actual_digest);
    std::fs::rename(&upload_path, &blob_path)
        .map_err(|e| format!("Failed to move upload to blob: {}", e))?;

    Ok(actual_digest)
}

pub(crate) fn delete_upload_session(
    org: &str,
    repo: &str,
    uuid: &str,
) -> Result<(), std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_uuid = sanitize_string(uuid);

    let upload_path = format!(
        "./tmp/uploads/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_uuid
    );
    std::fs::remove_file(upload_path)
}

pub(crate) fn delete_manifest(
    org: &str,
    repo: &str,
    reference: &str,
) -> Result<(), std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_reference = sanitize_string(reference);

    let manifest_path = format!(
        "./tmp/manifests/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_reference
    );

    if !std::path::Path::new(&manifest_path).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Manifest not found",
        ));
    }

    std::fs::remove_file(manifest_path)
}

pub(crate) fn delete_blob(org: &str, repo: &str, digest: &str) -> Result<(), std::io::Error> {
    let sanitized_org = sanitize_string(org);
    let sanitized_repo = sanitize_string(repo);
    let sanitized_digest = sanitize_string(digest);

    let blob_path = format!(
        "./tmp/blobs/{}/{}/{}",
        sanitized_org, sanitized_repo, sanitized_digest
    );

    if !std::path::Path::new(&blob_path).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Blob not found",
        ));
    }

    std::fs::remove_file(blob_path)
}

pub(crate) fn mount_blob(
    source_org: &str,
    source_repo: &str,
    target_org: &str,
    target_repo: &str,
    digest: &str,
) -> Result<(), std::io::Error> {
    let sanitized_source_org = sanitize_string(source_org);
    let sanitized_source_repo = sanitize_string(source_repo);
    let sanitized_target_org = sanitize_string(target_org);
    let sanitized_target_repo = sanitize_string(target_repo);
    let sanitized_digest = sanitize_string(digest);

    // Check if blob exists in source repository
    let source_path = format!(
        "./tmp/blobs/{}/{}/{}",
        sanitized_source_org, sanitized_source_repo, sanitized_digest
    );

    if !std::path::Path::new(&source_path).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Source blob not found",
        ));
    }

    // Create target directory
    let target_dir = format!(
        "./tmp/blobs/{}/{}",
        sanitized_target_org, sanitized_target_repo
    );
    std::fs::create_dir_all(&target_dir)?;

    // Create target path
    let target_path = format!("{}/{}", target_dir, sanitized_digest);

    // If target already exists, that's fine (already mounted)
    if std::path::Path::new(&target_path).exists() {
        return Ok(());
    }

    // Try hard link first (most efficient - no data duplication)
    if std::fs::hard_link(&source_path, &target_path).is_err() {
        // If hard link fails (cross-device), copy the file
        std::fs::copy(&source_path, &target_path)?;
    }

    Ok(())
}

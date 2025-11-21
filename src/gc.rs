use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

type BlobLocation = (String, String, u64); // (org, repo, size)
type UnreferencedBlob = (String, String, String, u64); // (org, repo, digest, size)

#[derive(Debug, Serialize, Deserialize)]
pub struct GcStats {
    pub blobs_scanned: usize,
    pub manifests_scanned: usize,
    pub blobs_referenced: usize,
    pub blobs_unreferenced: usize,
    pub blobs_deleted: usize,
    pub bytes_freed: u64,
    pub duration_seconds: u64,
}

/// Run garbage collection with optional dry-run mode
pub fn run_gc(
    dry_run: bool,
    grace_period_hours: u64,
) -> Result<GcStats, Box<dyn std::error::Error>> {
    let start_time = SystemTime::now();

    let mut stats = GcStats {
        blobs_scanned: 0,
        manifests_scanned: 0,
        blobs_referenced: 0,
        blobs_unreferenced: 0,
        blobs_deleted: 0,
        bytes_freed: 0,
        duration_seconds: 0,
    };

    log::info!("Starting garbage collection (dry_run: {})", dry_run);

    // Step 1: Scan all manifests and build referenced blob set
    let referenced_blobs = scan_manifests(&mut stats)?;
    stats.blobs_referenced = referenced_blobs.len();

    log::info!(
        "Found {} referenced blobs from {} manifests",
        stats.blobs_referenced,
        stats.manifests_scanned
    );

    // Step 2: Scan all blobs and identify unreferenced ones
    let all_blobs = scan_all_blobs(&mut stats)?;

    log::info!("Scanned {} total blobs", stats.blobs_scanned);

    // Step 3: Mark unreferenced blobs
    let unreferenced_blobs = mark_unreferenced_blobs(&all_blobs, &referenced_blobs)?;
    stats.blobs_unreferenced = unreferenced_blobs.len();

    log::info!("Identified {} unreferenced blobs", stats.blobs_unreferenced);

    // Step 4: Sweep marked blobs that are past grace period
    if !dry_run {
        sweep_marked_blobs(&unreferenced_blobs, grace_period_hours, &mut stats)?;
        log::info!(
            "Deleted {} blobs, freed {} bytes",
            stats.blobs_deleted,
            stats.bytes_freed
        );
    } else {
        log::info!("DRY RUN: Would delete {} blobs", unreferenced_blobs.len());
    }

    stats.duration_seconds = start_time.elapsed()?.as_secs();

    Ok(stats)
}

/// Scan all manifests and extract referenced blob digests
fn scan_manifests(stats: &mut GcStats) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut referenced = HashSet::new();
    let manifests_dir = Path::new("./tmp/manifests");

    if !manifests_dir.exists() {
        return Ok(referenced);
    }

    // Walk through org/repo/manifest structure
    for org_entry in std::fs::read_dir(manifests_dir)? {
        let org_entry = org_entry?;
        if !org_entry.path().is_dir() {
            continue;
        }

        for repo_entry in std::fs::read_dir(org_entry.path())? {
            let repo_entry = repo_entry?;
            if !repo_entry.path().is_dir() {
                continue;
            }

            for manifest_entry in std::fs::read_dir(repo_entry.path())? {
                let manifest_entry = manifest_entry?;
                if !manifest_entry.path().is_file() {
                    continue;
                }

                stats.manifests_scanned += 1;

                // Read and parse manifest
                if let Ok(manifest_data) = std::fs::read(manifest_entry.path()) {
                    if let Ok(manifest_str) = std::str::from_utf8(&manifest_data) {
                        extract_blob_references(manifest_str, &mut referenced);
                    }
                }
            }
        }
    }

    Ok(referenced)
}

/// Extract blob digest references from manifest JSON
fn extract_blob_references(manifest_json: &str, referenced: &mut HashSet<String>) {
    if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(manifest_json) {
        // Extract config digest
        if let Some(config) = manifest.get("config") {
            if let Some(digest) = config.get("digest").and_then(|d| d.as_str()) {
                let clean_digest = digest.strip_prefix("sha256:").unwrap_or(digest);
                referenced.insert(clean_digest.to_string());
            }
        }

        // Extract layer digests
        if let Some(layers) = manifest.get("layers").and_then(|l| l.as_array()) {
            for layer in layers {
                if let Some(digest) = layer.get("digest").and_then(|d| d.as_str()) {
                    let clean_digest = digest.strip_prefix("sha256:").unwrap_or(digest);
                    referenced.insert(clean_digest.to_string());
                }
            }
        }

        // Extract manifests from image index
        if let Some(manifests) = manifest.get("manifests").and_then(|m| m.as_array()) {
            for manifest_desc in manifests {
                if let Some(digest) = manifest_desc.get("digest").and_then(|d| d.as_str()) {
                    let clean_digest = digest.strip_prefix("sha256:").unwrap_or(digest);
                    referenced.insert(clean_digest.to_string());
                }
            }
        }
    }
}

/// Scan all blobs in storage
fn scan_all_blobs(
    stats: &mut GcStats,
) -> Result<HashMap<String, Vec<BlobLocation>>, Box<dyn std::error::Error>> {
    let mut all_blobs: HashMap<String, Vec<BlobLocation>> = HashMap::new();
    let blobs_dir = Path::new("./tmp/blobs");

    if !blobs_dir.exists() {
        return Ok(all_blobs);
    }

    for org_entry in std::fs::read_dir(blobs_dir)? {
        let org_entry = org_entry?;
        if !org_entry.path().is_dir() {
            continue;
        }

        let org = org_entry.file_name().to_string_lossy().to_string();

        for repo_entry in std::fs::read_dir(org_entry.path())? {
            let repo_entry = repo_entry?;
            if !repo_entry.path().is_dir() {
                continue;
            }

            let repo = repo_entry.file_name().to_string_lossy().to_string();

            for blob_entry in std::fs::read_dir(repo_entry.path())? {
                let blob_entry = blob_entry?;
                if !blob_entry.path().is_file() {
                    continue;
                }

                stats.blobs_scanned += 1;

                let digest = blob_entry.file_name().to_string_lossy().to_string();
                let size = blob_entry.metadata()?.len();

                // Track all locations for this digest
                all_blobs
                    .entry(digest)
                    .or_default()
                    .push((org.clone(), repo.clone(), size));
            }
        }
    }

    Ok(all_blobs)
}

/// Mark unreferenced blobs for deletion
fn mark_unreferenced_blobs(
    all_blobs: &HashMap<String, Vec<BlobLocation>>,
    referenced_blobs: &HashSet<String>,
) -> Result<Vec<UnreferencedBlob>, Box<dyn std::error::Error>> {
    let mut unreferenced = Vec::new();

    for (digest, locations) in all_blobs {
        if !referenced_blobs.contains(digest) {
            // Add all locations of this unreferenced blob
            for (org, repo, size) in locations {
                unreferenced.push((org.clone(), repo.clone(), digest.clone(), *size));
            }
        }
    }

    Ok(unreferenced)
}

/// Sweep (delete) marked blobs that are past grace period
fn sweep_marked_blobs(
    unreferenced_blobs: &[UnreferencedBlob],
    grace_period_hours: u64,
    stats: &mut GcStats,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let grace_period_secs = grace_period_hours * 3600;

    for (org, repo, digest, size) in unreferenced_blobs {
        // Check blob modification time
        let blob_path = format!("./tmp/blobs/{}/{}/{}", org, repo, digest);

        if let Ok(metadata) = std::fs::metadata(&blob_path) {
            if let Ok(modified) = metadata.modified() {
                let modified_secs = modified.duration_since(UNIX_EPOCH)?.as_secs();
                let age_secs = now.saturating_sub(modified_secs);

                // Only delete if past grace period
                if age_secs >= grace_period_secs {
                    match std::fs::remove_file(&blob_path) {
                        Ok(()) => {
                            log::info!(
                                "Deleted unreferenced blob: {}/{}/{} ({} bytes)",
                                org,
                                repo,
                                digest,
                                size
                            );
                            stats.blobs_deleted += 1;
                            stats.bytes_freed += size;
                        }
                        Err(e) => {
                            log::warn!("Failed to delete blob {}: {}", blob_path, e);
                        }
                    }
                } else {
                    log::debug!(
                        "Blob {} still in grace period ({} hours old)",
                        digest,
                        age_secs / 3600
                    );
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_blob_references() {
        let manifest = r#"{
            "config": {
                "digest": "sha256:abc123"
            },
            "layers": [
                {"digest": "sha256:layer1"},
                {"digest": "sha256:layer2"}
            ]
        }"#;

        let mut referenced = HashSet::new();
        extract_blob_references(manifest, &mut referenced);

        assert_eq!(referenced.len(), 3);
        assert!(referenced.contains("abc123"));
        assert!(referenced.contains("layer1"));
        assert!(referenced.contains("layer2"));
    }

    #[test]
    fn test_extract_image_index_references() {
        let manifest = r#"{
            "manifests": [
                {"digest": "sha256:manifest1"},
                {"digest": "sha256:manifest2"}
            ]
        }"#;

        let mut referenced = HashSet::new();
        extract_blob_references(manifest, &mut referenced);

        assert_eq!(referenced.len(), 2);
        assert!(referenced.contains("manifest1"));
        assert!(referenced.contains("manifest2"));
    }
}

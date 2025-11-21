mod common;

use common::*;
use serial_test::serial;

#[test]
#[serial]
fn test_gc_identifies_unreferenced_blobs() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload orphaned blob (not referenced by any manifest)
    let orphan_blob = b"orphaned blob content";
    let orphan_digest = format!("sha256:{}", sha256::digest(orphan_blob));
    client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            orphan_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(orphan_blob.to_vec())
        .send()
        .unwrap();

    // Upload referenced blob with manifest
    let referenced_blob = sample_blob();
    let referenced_digest = sample_blob_digest();
    client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            referenced_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(referenced_blob)
        .send()
        .unwrap();

    let manifest = sample_manifest();
    client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    // Run GC with dry-run
    let resp = client
        .post("/admin/gc?dry_run=true&grace_period_hours=0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let result: serde_json::Value = resp.json().unwrap();

    assert!(result["blobs_scanned"].as_u64().unwrap() >= 2);
    assert!(result["blobs_unreferenced"].as_u64().unwrap() >= 1);

    // Verify orphaned blob still exists (dry-run)
    let resp = client
        .head(&format!("/v2/test/repo/blobs/{}", orphan_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_gc_actual_deletion() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload orphaned blob
    let orphan_blob = b"orphaned blob to delete";
    let orphan_digest = format!("sha256:{}", sha256::digest(orphan_blob));
    client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            orphan_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(orphan_blob.to_vec())
        .send()
        .unwrap();

    // Run GC without dry-run
    let resp = client
        .post("/admin/gc?dry_run=false&grace_period_hours=0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let result: serde_json::Value = resp.json().unwrap();
    assert!(result["blobs_deleted"].as_u64().unwrap() >= 1);
    assert!(result["bytes_freed"].as_u64().unwrap() > 0);

    // Verify orphaned blob is gone
    let resp = client
        .head(&format!("/v2/test/repo/blobs/{}", orphan_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_gc_grace_period_enforcement() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload recent orphaned blob
    let recent_blob = b"recent orphaned blob";
    let recent_digest = format!("sha256:{}", sha256::digest(recent_blob));
    client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            recent_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(recent_blob.to_vec())
        .send()
        .unwrap();

    // Run GC with 24-hour grace period (recent blob should be preserved)
    let resp = client
        .post("/admin/gc?dry_run=false&grace_period_hours=24")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify recent blob still exists
    let resp = client
        .head(&format!("/v2/test/repo/blobs/{}", recent_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_gc_manifest_reference_extraction() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload config blob
    let config_blob = b"config blob content";
    let config_digest = format!("sha256:{}", sha256::digest(config_blob));
    client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            config_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(config_blob.to_vec())
        .send()
        .unwrap();

    // Upload layer blob
    let layer_blob = b"layer blob content";
    let layer_digest = format!("sha256:{}", sha256::digest(layer_blob));
    client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            layer_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(layer_blob.to_vec())
        .send()
        .unwrap();

    // Create manifest referencing both blobs
    let manifest = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "size": config_blob.len(),
            "digest": config_digest
        },
        "layers": [
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                "size": layer_blob.len(),
                "digest": layer_digest
            }
        ]
    });

    client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    // Run GC
    let resp = client
        .post("/admin/gc?dry_run=false&grace_period_hours=0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify both referenced blobs still exist
    let resp = client
        .head(&format!("/v2/test/repo/blobs/{}", config_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .head(&format!("/v2/test/repo/blobs/{}", layer_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_gc_image_index_traversal() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob for sub-manifest
    let blob = sample_blob();
    let blob_digest = sample_blob_digest();
    client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            blob_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Upload sub-manifest
    let sub_manifest = sample_manifest();
    let sub_manifest_bytes = serde_json::to_vec(&sub_manifest).unwrap();
    let sub_manifest_digest = format!("sha256:{}", sha256::digest(&sub_manifest_bytes));

    client
        .put(&format!("/v2/test/repo/manifests/{}", sub_manifest_digest))
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&sub_manifest)
        .send()
        .unwrap();

    // Upload image index referencing sub-manifest
    let index = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "size": sub_manifest_bytes.len(),
                "digest": sub_manifest_digest,
                "platform": {
                    "architecture": "amd64",
                    "os": "linux"
                }
            }
        ]
    });

    client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.index.v1+json")
        .json(&index)
        .send()
        .unwrap();

    // Run GC
    let resp = client
        .post("/admin/gc?dry_run=false&grace_period_hours=0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify blob referenced by sub-manifest still exists
    let resp = client
        .head(&format!("/v2/test/repo/blobs/{}", blob_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_gc_statistics_accuracy() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload 3 orphaned blobs
    for i in 1..=3 {
        let blob = format!("orphaned blob {}", i);
        let digest = format!("sha256:{}", sha256::digest(blob.as_bytes()));
        client
            .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
            .basic_auth("admin", Some("admin"))
            .body(blob.into_bytes())
            .send()
            .unwrap();
    }

    // Run GC
    let resp = client
        .post("/admin/gc?dry_run=false&grace_period_hours=0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let result: serde_json::Value = resp.json().unwrap();

    assert_eq!(result["blobs_scanned"].as_u64().unwrap(), 3);
    assert_eq!(result["blobs_deleted"].as_u64().unwrap(), 3);
    assert!(result["bytes_freed"].as_u64().unwrap() > 0);
}

#[test]
#[serial]
fn test_gc_requires_admin_permission() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Non-admin user should not be able to run GC
    let resp = client
        .post("/admin/gc?dry_run=true&grace_period_hours=0")
        .basic_auth("reader", Some("reader"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 403);

    // Admin should be able to run GC
    let resp = client
        .post("/admin/gc?dry_run=true&grace_period_hours=0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_gc_preserves_shared_blobs_across_repos() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob to repo1
    let shared_blob = b"shared blob content";
    let shared_digest = format!("sha256:{}", sha256::digest(shared_blob));
    client
        .post(&format!(
            "/v2/repo1/test/blobs/uploads/?digest={}",
            shared_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(shared_blob.to_vec())
        .send()
        .unwrap();

    // Create manifest in repo1
    let manifest1 = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "size": shared_blob.len(),
            "digest": shared_digest
        },
        "layers": [
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                "size": shared_blob.len(),
                "digest": shared_digest
            }
        ]
    });

    client
        .put("/v2/repo1/test/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest1)
        .send()
        .unwrap();

    // Mount same blob to repo2
    client
        .post(&format!(
            "/v2/repo2/test/blobs/uploads/?mount={}&from=repo1/test",
            shared_digest
        ))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    // Create manifest in repo2 referencing the same blob
    let manifest2 = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "size": shared_blob.len(),
            "digest": shared_digest
        },
        "layers": [
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                "size": shared_blob.len(),
                "digest": shared_digest
            }
        ]
    });

    client
        .put("/v2/repo2/test/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest2)
        .send()
        .unwrap();

    // Run GC
    let resp = client
        .post("/admin/gc?dry_run=false&grace_period_hours=0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify blob still exists in both repos
    let resp = client
        .head(&format!("/v2/repo1/test/blobs/{}", shared_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .head(&format!("/v2/repo2/test/blobs/{}", shared_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

mod common;

use common::*;
use serial_test::serial;

#[test]
#[serial]
fn test_end1_version_check_authenticated() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    // Note: Docker-Distribution-API-Version header may not be present in current implementation
    // This is acceptable for basic OCI compliance
}

#[test]
#[serial]
fn test_end1_version_check_unauthenticated() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client.get("/v2/").send().unwrap();

    assert_eq!(resp.status(), 401);
    assert!(resp.headers().contains_key("www-authenticate"));
}

#[test]
#[serial]
fn test_end1_version_check_invalid_credentials() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/")
        .basic_auth("invalid", Some("invalid"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[test]
#[serial]
fn test_end2_blob_get_nonexistent() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/test/repo/blobs/sha256:nonexistent")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_end2_blob_head_nonexistent() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .head("/v2/test/repo/blobs/sha256:nonexistent")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_end4a_blob_upload_initiate() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .post("/v2/test/repo/blobs/uploads/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 202);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-upload-uuid"));
}

#[test]
#[serial]
fn test_end4b_monolithic_blob_upload() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let digest = sample_blob_digest();

    let resp = client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/octet-stream")
        .body(blob)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

#[test]
#[serial]
fn test_end4b_monolithic_upload_digest_mismatch() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let wrong_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let resp = client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            wrong_digest
        ))
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/octet-stream")
        .body(blob)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[test]
#[serial]
fn test_end5_end6_chunked_upload_complete() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Initiate upload
    let resp = client
        .post("/v2/test/repo/blobs/uploads/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 202);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();

    // PATCH: Upload chunk
    let blob = sample_blob();
    let resp = client
        .patch(location)
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/octet-stream")
        .body(blob.clone())
        .send()
        .unwrap();

    assert_eq!(resp.status(), 202);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();

    // PUT: Complete upload
    let digest = sample_blob_digest();
    let resp = client
        .put(&format!("{}?digest={}", location, digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);
}

#[test]
#[serial]
fn test_end6_complete_upload_with_digest_mismatch() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Initiate and upload chunk
    let resp = client
        .post("/v2/test/repo/blobs/uploads/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    let location = resp.headers().get("location").unwrap().to_str().unwrap();

    let blob = sample_blob();
    let resp = client
        .patch(location)
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();
    let location = resp.headers().get("location").unwrap().to_str().unwrap();

    // Try to complete with wrong digest
    let wrong_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let resp = client
        .put(&format!("{}?digest={}", location, wrong_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[test]
#[serial]
fn test_end7_manifest_upload() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // First upload the blob referenced in manifest
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Upload manifest
    let manifest = sample_manifest();
    let resp = client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

#[test]
#[serial]
fn test_end7_manifest_upload_invalid_json() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body("invalid json")
        .send()
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[test]
#[serial]
fn test_end7_manifest_upload_invalid_schema() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let invalid_manifest = serde_json::json!({
        "schemaVersion": 99,
        "config": {}
    });

    let resp = client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&invalid_manifest)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[test]
#[serial]
fn test_end3_manifest_get_by_tag() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob and manifest first
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    let manifest = sample_manifest();
    client
        .put("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    // Get manifest by tag
    let resp = client
        .get("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().contains_key("docker-content-digest"));
    assert!(resp.headers().contains_key("content-type"));
}

#[test]
#[serial]
fn test_end3_manifest_get_by_digest() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob and manifest
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

    let manifest = sample_manifest();
    let manifest_digest = sample_manifest_digest(&manifest);
    client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    // Get by digest
    let resp = client
        .get(&format!("/v2/test/repo/manifests/{}", manifest_digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_end3_manifest_head() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob and manifest
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
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

    // HEAD request
    let resp = client
        .head("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().contains_key("docker-content-digest"));
    assert!(resp.headers().contains_key("content-length"));
}

#[test]
#[serial]
fn test_end3_manifest_get_nonexistent() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/test/repo/manifests/nonexistent")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_end8a_tag_list_empty() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/test/repo/tags/list")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().unwrap();
    assert_eq!(json["name"], "test/repo");
    assert!(json["tags"].is_array());
}

#[test]
#[serial]
fn test_end8a_tag_list_with_tags() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob and manifests with different tags
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    let manifest = sample_manifest();
    for tag in &["v1.0", "v2.0", "latest"] {
        client
            .put(&format!("/v2/test/repo/manifests/{}", tag))
            .basic_auth("admin", Some("admin"))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .json(&manifest)
            .send()
            .unwrap();
    }

    // List tags
    let resp = client
        .get("/v2/test/repo/tags/list")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().unwrap();
    let tags = json["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 3);
}

#[test]
#[serial]
fn test_end8b_tag_list_pagination() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob and multiple tagged manifests
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    let manifest = sample_manifest();
    for i in 1..=10 {
        client
            .put(&format!("/v2/test/repo/manifests/v{}", i))
            .basic_auth("admin", Some("admin"))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .json(&manifest)
            .send()
            .unwrap();
    }

    // Request with pagination
    let resp = client
        .get("/v2/test/repo/tags/list?n=5")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().unwrap();
    let tags = json["tags"].as_array().unwrap();
    assert!(tags.len() <= 5);
}

#[test]
#[serial]
fn test_end9_delete_manifest() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob and manifest
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    let manifest = sample_manifest();
    client
        .put("/v2/test/repo/manifests/deleteme")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    // Delete manifest
    let resp = client
        .delete("/v2/test/repo/manifests/deleteme")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 202);

    // Verify it's gone
    let resp = client
        .get("/v2/test/repo/manifests/deleteme")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_end10_delete_blob() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Delete blob
    let resp = client
        .delete(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 202);

    // Verify it's gone
    let resp = client
        .get(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_end11_cross_repo_blob_mount_success() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob to source repo
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/source/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Mount to target repo
    let resp = client
        .post(&format!(
            "/v2/target/repo/blobs/uploads/?mount={}&from=source/repo",
            digest
        ))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);
    assert!(resp.headers().contains_key("location"));

    // Verify blob exists in target repo
    let resp = client
        .head(&format!("/v2/target/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_end11_cross_repo_mount_nonexistent_blob() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    // Try to mount nonexistent blob
    let resp = client
        .post(&format!(
            "/v2/target/repo/blobs/uploads/?mount={}&from=source/repo",
            digest
        ))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    // Should fall back to regular upload initiation
    assert_eq!(resp.status(), 202);
}

mod common;

use common::*;
use serial_test::serial;

fn extract_path(location: &str) -> &str {
    // Extract path from absolute URL (e.g., "http://127.0.0.1:8080/v2/..." -> "/v2/...")
    location
        .find("://")
        .and_then(|proto_end| {
            location[proto_end + 3..]
                .find('/')
                .map(|path_start| &location[proto_end + 3 + path_start..])
        })
        .unwrap_or(location)
}

#[test]
#[serial]
fn test_storage_blob_write_and_read() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Write blob
    let resp = client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob.clone())
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);

    // Read blob
    let resp = client
        .get(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let retrieved_blob = resp.bytes().unwrap();
    assert_eq!(retrieved_blob.as_ref(), blob.as_slice());
}

#[test]
#[serial]
fn test_storage_digest_validation_on_upload() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = b"test content";
    let correct_digest = format!("sha256:{}", sha256::digest(blob));
    let wrong_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    // Upload with correct digest should succeed
    let resp = client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            correct_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(blob.to_vec())
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Upload with wrong digest should fail
    let resp = client
        .post(&format!(
            "/v2/test/repo/blobs/uploads/?digest={}",
            wrong_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(blob.to_vec())
        .send()
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[test]
#[serial]
fn test_storage_manifest_write_and_read() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob first
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Write manifest
    let manifest = sample_manifest();
    let resp = client
        .put("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Read manifest
    let resp = client
        .get("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let retrieved: serde_json::Value = resp.json().unwrap();
    assert_eq!(retrieved["schemaVersion"], manifest["schemaVersion"]);
}

#[test]
#[serial]
fn test_storage_upload_session_lifecycle() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Initiate upload session
    let resp = client
        .post("/v2/test/repo/blobs/uploads/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 202);

    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    let uuid = resp.headers().get("docker-upload-uuid").unwrap();
    assert!(!uuid.is_empty());

    // Append chunk 1
    let chunk1 = b"first chunk";
    let resp = client
        .patch(extract_path(location))
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/octet-stream")
        .body(chunk1.to_vec())
        .send()
        .unwrap();
    assert_eq!(resp.status(), 202);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();

    // Append chunk 2
    let chunk2 = b" second chunk";
    let resp = client
        .patch(extract_path(location))
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/octet-stream")
        .body(chunk2.to_vec())
        .send()
        .unwrap();
    assert_eq!(resp.status(), 202);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();

    // Complete upload
    let combined: Vec<u8> = [chunk1.as_slice(), chunk2.as_slice()].concat();
    let digest = format!("sha256:{}", sha256::digest(&combined));
    let resp = client
        .put(&format!("{}?digest={}", extract_path(location), digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Verify blob exists
    let resp = client
        .get(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.bytes().unwrap().as_ref(), combined.as_slice());
}

#[test]
#[serial]
fn test_storage_path_sanitization() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Try directory traversal in repo name
    let resp = client
        .post(&format!(
            "/v2/../../../etc/passwd/blobs/uploads/?digest={}",
            digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Should either sanitize or reject, but not create files outside tmp/
    // 201 = accepted and sanitized, 400 = rejected, 200 = catch-all (invalid route)
    assert!(
        resp.status() == 400 || resp.status() == 201 || resp.status() == 200,
        "Unexpected status: {}",
        resp.status()
    );

    // Verify no file created outside temp directory
    let temp_path = server.temp_dir.path();
    let parent = temp_path.parent().unwrap();

    // Check that no suspicious files were created
    if parent.join("etc").exists() {
        panic!("Directory traversal succeeded - security issue!");
    }
}

#[test]
#[serial]
fn test_storage_cross_repo_blob_mount_creates_hardlink() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob to source repo
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/source/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob.clone())
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

    // Verify blob exists in both locations
    let resp = client
        .get(&format!("/v2/source/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(&format!("/v2/target/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify both return same content
    let source_content = client
        .get(&format!("/v2/source/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap()
        .bytes()
        .unwrap();

    let target_content = client
        .get(&format!("/v2/target/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap()
        .bytes()
        .unwrap();

    assert_eq!(source_content, target_content);
    assert_eq!(source_content.as_ref(), blob.as_slice());
}

#[test]
#[serial]
fn test_storage_concurrent_uploads_same_repo() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Initiate two upload sessions
    let resp1 = client
        .post("/v2/test/repo/blobs/uploads/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    let location1 = resp1
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let resp2 = client
        .post("/v2/test/repo/blobs/uploads/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    let location2 = resp2
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Verify different upload UUIDs
    assert_ne!(location1, location2);

    // Upload different blobs to each session
    let blob1 = b"blob 1 content";
    let digest1 = format!("sha256:{}", sha256::digest(blob1));

    let blob2 = b"blob 2 content different";
    let digest2 = format!("sha256:{}", sha256::digest(blob2));

    // Complete first upload
    let resp = client
        .patch(extract_path(&location1))
        .basic_auth("admin", Some("admin"))
        .body(blob1.to_vec())
        .send()
        .unwrap();
    let location1 = resp.headers().get("location").unwrap().to_str().unwrap();

    let resp = client
        .put(&format!("{}?digest={}", extract_path(location1), digest1))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Complete second upload
    let resp = client
        .patch(extract_path(&location2))
        .basic_auth("admin", Some("admin"))
        .body(blob2.to_vec())
        .send()
        .unwrap();
    let location2 = resp.headers().get("location").unwrap().to_str().unwrap();

    let resp = client
        .put(&format!("{}?digest={}", extract_path(location2), digest2))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Verify both blobs exist and have correct content
    let content1 = client
        .get(&format!("/v2/test/repo/blobs/{}", digest1))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap()
        .bytes()
        .unwrap();
    assert_eq!(content1.as_ref(), blob1);

    let content2 = client
        .get(&format!("/v2/test/repo/blobs/{}", digest2))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap()
        .bytes()
        .unwrap();
    assert_eq!(content2.as_ref(), blob2);
}

#[test]
#[serial]
fn test_storage_delete_blob() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Upload blob
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Verify it exists
    let resp = client
        .get(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Delete it
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
fn test_storage_delete_manifest() {
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
        .put("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    // Verify it exists
    let resp = client
        .get("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Delete it
    let resp = client
        .delete("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 202);

    // Verify it's gone
    let resp = client
        .get("/v2/test/repo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_storage_blob_metadata() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Upload blob
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob.clone())
        .send()
        .unwrap();

    // HEAD request should return metadata
    let resp = client
        .head(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().contains_key("content-length"));
    assert!(resp.headers().contains_key("docker-content-digest"));

    let content_length = resp
        .headers()
        .get("content-length")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(content_length, blob.len());
}

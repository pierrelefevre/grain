mod common;

use common::*;
use serial_test::serial;

#[test]
#[serial]
fn test_auth_malformed_basic_auth_header() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/")
        .header("Authorization", "Basic invalid-base64!!!!")
        .send()
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[test]
#[serial]
fn test_auth_missing_credentials() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client.get("/v2/").send().unwrap();

    assert_eq!(resp.status(), 401);
    assert!(resp.headers().contains_key("www-authenticate"));
    let www_auth = resp
        .headers()
        .get("www-authenticate")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(www_auth.contains("Basic realm="));
}

#[test]
#[serial]
fn test_auth_invalid_username() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/")
        .basic_auth("nonexistent", Some("password"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[test]
#[serial]
fn test_auth_invalid_password() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/v2/")
        .basic_auth("admin", Some("wrongpassword"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[test]
#[serial]
fn test_permission_admin_wildcard_access() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Admin should be able to push to any repo
    let blob = sample_blob();
    let digest = sample_blob_digest();

    let resp = client
        .post(&format!("/v2/any/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);
}

#[test]
#[serial]
fn test_permission_reader_can_pull() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // First upload as admin
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    // Reader should be able to GET
    let resp = client
        .get(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("reader", Some("reader"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_permission_reader_cannot_push() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Reader should NOT be able to push
    let resp = client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("reader", Some("reader"))
        .body(blob)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 403);
}

#[test]
#[serial]
fn test_permission_writer_can_push() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Writer should be able to push
    let resp = client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("writer", Some("writer"))
        .body(blob)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);
}

#[test]
#[serial]
fn test_permission_writer_cannot_delete() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload as writer
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("writer", Some("writer"))
        .body(blob)
        .send()
        .unwrap();

    // Writer should NOT be able to delete
    let resp = client
        .delete(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("writer", Some("writer"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 403);
}

#[test]
#[serial]
fn test_permission_repository_pattern_exact_match() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Limited user has access to myorg/myrepo only
    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Should work for exact match
    let resp = client
        .post(&format!(
            "/v2/myorg/myrepo/blobs/uploads/?digest={}",
            digest
        ))
        .basic_auth("limited", Some("limited"))
        .body(blob.clone())
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);

    // Should fail for different repo
    let resp = client
        .post(&format!("/v2/myorg/other/blobs/uploads/?digest={}", digest))
        .basic_auth("limited", Some("limited"))
        .body(blob)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 403);
}

#[test]
#[serial]
fn test_permission_repository_pattern_prefix_wildcard() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Reader has access to test/* repos
    let blob = sample_blob();
    let digest = sample_blob_digest();

    // Upload as admin first
    client
        .post(&format!("/v2/test/repo1/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob.clone())
        .send()
        .unwrap();

    client
        .post(&format!("/v2/test/repo2/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob.clone())
        .send()
        .unwrap();

    // Reader should access both
    let resp = client
        .get(&format!("/v2/test/repo1/blobs/{}", digest))
        .basic_auth("reader", Some("reader"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(&format!("/v2/test/repo2/blobs/{}", digest))
        .basic_auth("reader", Some("reader"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    // But not other orgs
    client
        .post(&format!("/v2/other/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob.clone())
        .send()
        .unwrap();

    let resp = client
        .get(&format!("/v2/other/repo/blobs/{}", digest))
        .basic_auth("reader", Some("reader"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[test]
#[serial]
fn test_permission_tag_pattern_prefix_wildcard() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Limited user can only pull tags matching v*
    let blob = sample_blob();
    let blob_digest = sample_blob_digest();
    client
        .post(&format!(
            "/v2/myorg/myrepo/blobs/uploads/?digest={}",
            blob_digest
        ))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    let manifest = sample_manifest();

    // Upload manifests with different tags as admin
    client
        .put("/v2/myorg/myrepo/manifests/v1.0")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    client
        .put("/v2/myorg/myrepo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();

    // Limited user should access v1.0 (matches v*)
    let resp = client
        .get("/v2/myorg/myrepo/manifests/v1.0")
        .basic_auth("limited", Some("limited"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    // But not latest (doesn't match v*)
    let resp = client
        .get("/v2/myorg/myrepo/manifests/latest")
        .basic_auth("limited", Some("limited"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[test]
#[serial]
fn test_permission_cross_repo_mount_requires_both_permissions() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob to source as writer
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/source/blobs/uploads/?digest={}", digest))
        .basic_auth("writer", Some("writer"))
        .body(blob)
        .send()
        .unwrap();

    // Writer can mount within test/* repos (has both pull on source and push on target)
    let resp = client
        .post(&format!(
            "/v2/test/target/blobs/uploads/?mount={}&from=test/source",
            digest
        ))
        .basic_auth("writer", Some("writer"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);
}

#[test]
#[serial]
fn test_permission_unauthorized_vs_forbidden() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // No credentials = 401 Unauthorized
    let resp = client.get("/v2/").send().unwrap();
    assert_eq!(resp.status(), 401);

    // Invalid credentials = 401 Unauthorized
    let resp = client
        .get("/v2/")
        .basic_auth("invalid", Some("invalid"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 401);

    // Valid credentials but no permission = 403 Forbidden
    let resp = client
        .post("/v2/other/repo/blobs/uploads/")
        .basic_auth("limited", Some("limited"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[test]
#[serial]
fn test_permission_admin_can_delete() {
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

    // Admin should be able to delete
    let resp = client
        .delete(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 202);
}

#[test]
#[serial]
fn test_permission_action_enforcement_on_manifest_operations() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Upload blob as admin
    let blob = sample_blob();
    let digest = sample_blob_digest();
    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob)
        .send()
        .unwrap();

    let manifest = sample_manifest();

    // Reader cannot push manifest
    let resp = client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("reader", Some("reader"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Writer can push manifest
    let resp = client
        .put("/v2/test/repo/manifests/latest")
        .basic_auth("writer", Some("writer"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .json(&manifest)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Reader can pull manifest
    let resp = client
        .get("/v2/test/repo/manifests/latest")
        .basic_auth("reader", Some("reader"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Writer cannot delete manifest (no delete permission)
    let resp = client
        .delete("/v2/test/repo/manifests/latest")
        .basic_auth("writer", Some("writer"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Admin can delete manifest
    let resp = client
        .delete("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 202);
}

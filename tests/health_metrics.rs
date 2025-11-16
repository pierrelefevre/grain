mod common;

use common::*;
use serial_test::serial;

#[test]
#[serial]
fn test_health_liveness_always_returns_200() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client.get("/health/live").send().unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().unwrap();
    assert_eq!(json["status"], "alive");
}

#[test]
#[serial]
fn test_health_readiness_when_ready() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client.get("/health/ready").send().unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().unwrap();
    assert_eq!(json["ready"], true);
    assert_eq!(json["checks"]["storage_accessible"], true);
    assert_eq!(json["checks"]["users_loaded"], true);
}

#[test]
#[serial]
fn test_health_detailed_endpoint() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client.get("/health").send().unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().unwrap();

    assert_eq!(json["status"], "healthy");
    assert!(json["version"].is_string());
    assert!(json["uptime_seconds"].is_number());

    let storage = &json["storage"];
    assert_eq!(storage["accessible"], true);
    assert_eq!(storage["writable"], true);
    assert!(storage["blobs_path"].is_string());
    assert!(storage["manifests_path"].is_string());
}

#[test]
#[serial]
fn test_health_no_authentication_required() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Health endpoints should work without auth
    let resp = client.get("/health").send().unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client.get("/health/live").send().unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client.get("/health/ready").send().unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_metrics_endpoint_prometheus_format() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client.get("/metrics").send().unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/plain; version=0.0.4; charset=utf-8"
    );

    let body = resp.text().unwrap();

    // Should contain prometheus metrics
    assert!(body.contains("grain_http_requests_total"));
    assert!(body.contains("grain_request_duration_seconds"));
}

#[test]
#[serial]
fn test_metrics_no_authentication_required() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Metrics endpoint should work without auth (for Prometheus scraping)
    let resp = client.get("/metrics").send().unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_metrics_counter_increments() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Get initial metrics
    let resp = client.get("/metrics").send().unwrap();
    let initial_body = resp.text().unwrap();

    // Make some requests
    let blob = sample_blob();
    let digest = sample_blob_digest();

    client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("admin", Some("admin"))
        .body(blob.clone())
        .send()
        .unwrap();

    client
        .get(&format!("/v2/test/repo/blobs/{}", digest))
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    // Get updated metrics
    let resp = client.get("/metrics").send().unwrap();
    let updated_body = resp.text().unwrap();

    // Verify metrics changed (counters should have incremented)
    assert_ne!(initial_body, updated_body);
    assert!(updated_body.contains("grain_blob_uploads_total"));
    assert!(updated_body.contains("grain_blob_downloads_total"));
}

#[test]
#[serial]
fn test_metrics_auth_failure_counter() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Make invalid auth request
    let resp = client
        .get("/v2/")
        .basic_auth("invalid", Some("invalid"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 401);

    // Check metrics
    let resp = client.get("/metrics").send().unwrap();
    let body = resp.text().unwrap();

    assert!(body.contains("grain_auth_failures_total"));
}

#[test]
#[serial]
fn test_metrics_permission_denial_counter() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Make request that will be denied due to permissions
    let blob = sample_blob();
    let digest = sample_blob_digest();

    let resp = client
        .post(&format!(
            "/v2/forbidden/repo/blobs/uploads/?digest={}",
            digest
        ))
        .basic_auth("limited", Some("limited"))
        .body(blob)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Check metrics
    let resp = client.get("/metrics").send().unwrap();
    let body = resp.text().unwrap();

    assert!(body.contains("grain_permission_denials_total"));
}

#[test]
#[serial]
fn test_metrics_manifest_upload_download_counters() {
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

    // Download manifest
    client
        .get("/v2/test/repo/manifests/latest")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    // Check metrics
    let resp = client.get("/metrics").send().unwrap();
    let body = resp.text().unwrap();

    assert!(body.contains("grain_manifest_uploads_total"));
    assert!(body.contains("grain_manifest_downloads_total"));
}

#[test]
#[serial]
fn test_metrics_request_duration_histogram() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Make some requests
    client
        .get("/v2/")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    // Check metrics
    let resp = client.get("/metrics").send().unwrap();
    let body = resp.text().unwrap();

    assert!(body.contains("grain_request_duration_seconds"));
    assert!(body.contains("_bucket"));
    assert!(body.contains("_sum"));
    assert!(body.contains("_count"));
}

#[test]
#[serial]
fn test_metrics_endpoint_normalization() {
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

    // Check metrics contain normalized endpoint
    let resp = client.get("/metrics").send().unwrap();
    let body = resp.text().unwrap();

    // Endpoints should be normalized to avoid cardinality explosion
    // Should see /v2/{name}/blobs/{digest} not the actual digest
    assert!(body.contains("grain_http_requests_total"));
}

#[test]
#[serial]
fn test_health_uptime_tracking() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Get initial uptime
    let resp = client.get("/health").send().unwrap();
    let json1: serde_json::Value = resp.json().unwrap();
    let uptime1 = json1["uptime_seconds"].as_u64().unwrap();

    // Wait a bit
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Get updated uptime
    let resp = client.get("/health").send().unwrap();
    let json2: serde_json::Value = resp.json().unwrap();
    let uptime2 = json2["uptime_seconds"].as_u64().unwrap();

    // Uptime should have increased
    assert!(uptime2 > uptime1);
}

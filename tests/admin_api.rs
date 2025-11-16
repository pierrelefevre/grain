mod common;

use common::*;
use serial_test::serial;

#[test]
#[serial]
fn test_admin_list_users() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .get("/admin/users")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().unwrap();

    let users = json["users"].as_array().unwrap();
    assert!(users.len() >= 4); // admin, reader, writer, limited

    // Verify admin user exists
    let admin_user = users.iter().find(|u| u["username"] == "admin");
    assert!(admin_user.is_some());
}

#[test]
#[serial]
fn test_admin_create_user() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let new_user = serde_json::json!({
        "username": "newuser",
        "password": "newpass",
        "permissions": []
    });

    let resp = client
        .post("/admin/users")
        .basic_auth("admin", Some("admin"))
        .json(&new_user)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);

    // Verify user can authenticate
    let resp = client
        .get("/v2/")
        .basic_auth("newuser", Some("newpass"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_admin_create_duplicate_user() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let duplicate_user = serde_json::json!({
        "username": "admin",
        "password": "newpass",
        "permissions": []
    });

    let resp = client
        .post("/admin/users")
        .basic_auth("admin", Some("admin"))
        .json(&duplicate_user)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 409); // Conflict
}

#[test]
#[serial]
fn test_admin_delete_user() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Create user first
    let new_user = serde_json::json!({
        "username": "todelete",
        "password": "pass",
        "permissions": []
    });

    client
        .post("/admin/users")
        .basic_auth("admin", Some("admin"))
        .json(&new_user)
        .send()
        .unwrap();

    // Delete user
    let resp = client
        .delete("/admin/users/todelete")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify user cannot authenticate
    let resp = client
        .get("/v2/")
        .basic_auth("todelete", Some("pass"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[test]
#[serial]
fn test_admin_delete_nonexistent_user() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let resp = client
        .delete("/admin/users/nonexistent")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[test]
#[serial]
fn test_admin_add_permission() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Create user with no permissions
    let new_user = serde_json::json!({
        "username": "testperm",
        "password": "pass",
        "permissions": []
    });

    client
        .post("/admin/users")
        .basic_auth("admin", Some("admin"))
        .json(&new_user)
        .send()
        .unwrap();

    // User should not be able to push
    let blob = sample_blob();
    let digest = sample_blob_digest();
    let resp = client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("testperm", Some("pass"))
        .body(blob.clone())
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Add permission
    let permission = serde_json::json!({
        "username": "testperm",
        "repository": "test/*",
        "tag": "*",
        "actions": ["pull", "push"]
    });

    let resp = client
        .post("/admin/permissions")
        .basic_auth("admin", Some("admin"))
        .json(&permission)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 200);

    // User should now be able to push
    let resp = client
        .post(&format!("/v2/test/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("testperm", Some("pass"))
        .body(blob)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);
}

#[test]
#[serial]
fn test_admin_requires_admin_permission() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Non-admin users should not access admin endpoints
    let resp = client
        .get("/admin/users")
        .basic_auth("reader", Some("reader"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);

    let resp = client
        .post("/admin/users")
        .basic_auth("writer", Some("writer"))
        .json(&serde_json::json!({"username": "test", "password": "test", "permissions": []}))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 403);

    // Admin should have access
    let resp = client
        .get("/admin/users")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
#[serial]
fn test_admin_api_requires_authentication() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // No auth should fail
    let resp = client.get("/admin/users").send().unwrap();
    assert_eq!(resp.status(), 401);

    // Invalid credentials should fail
    let resp = client
        .get("/admin/users")
        .basic_auth("invalid", Some("invalid"))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[test]
#[serial]
fn test_admin_create_user_with_permissions() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    let new_user = serde_json::json!({
        "username": "fulluser",
        "password": "pass",
        "permissions": [
            {
                "repository": "myorg/*",
                "tag": "*",
                "actions": ["pull", "push"]
            }
        ]
    });

    let resp = client
        .post("/admin/users")
        .basic_auth("admin", Some("admin"))
        .json(&new_user)
        .send()
        .unwrap();

    assert_eq!(resp.status(), 201);

    // Verify permissions work
    let blob = sample_blob();
    let digest = sample_blob_digest();
    let resp = client
        .post(&format!("/v2/myorg/repo/blobs/uploads/?digest={}", digest))
        .basic_auth("fulluser", Some("pass"))
        .body(blob)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);
}

#[test]
#[serial]
fn test_admin_user_persistence() {
    let mut server = TestServer::new();
    server.start();
    let client = server.client();

    // Create user
    let new_user = serde_json::json!({
        "username": "persistent",
        "password": "pass",
        "permissions": []
    });

    client
        .post("/admin/users")
        .basic_auth("admin", Some("admin"))
        .json(&new_user)
        .send()
        .unwrap();

    // Verify user exists in list
    let resp = client
        .get("/admin/users")
        .basic_auth("admin", Some("admin"))
        .send()
        .unwrap();

    let json: serde_json::Value = resp.json().unwrap();
    let users = json["users"].as_array().unwrap();
    let persistent_user = users.iter().find(|u| u["username"] == "persistent");
    assert!(persistent_user.is_some());
}

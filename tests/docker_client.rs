// Docker client integration tests
// These tests require Docker to be installed and running
// Enabled with --features docker-tests

#![cfg(feature = "docker-tests")]

mod common;

use common::*;
use serial_test::serial;
use std::process::Command;

fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn docker_login(registry: &str, username: &str, password: &str) -> bool {
    let output = Command::new("docker")
        .args(["login", registry, "-u", username, "-p", password])
        .output()
        .expect("Failed to run docker login");

    output.status.success()
}

fn docker_logout(registry: &str) {
    let _ = Command::new("docker").args(["logout", registry]).output();
}

fn docker_tag(source: &str, target: &str) -> bool {
    let output = Command::new("docker")
        .args(["tag", source, target])
        .output()
        .expect("Failed to run docker tag");

    output.status.success()
}

fn docker_push(image: &str) -> bool {
    let output = Command::new("docker")
        .args(["push", image])
        .output()
        .expect("Failed to run docker push");

    output.status.success()
}

fn docker_pull(image: &str) -> bool {
    let output = Command::new("docker")
        .args(["pull", image])
        .output()
        .expect("Failed to run docker pull");

    output.status.success()
}

fn docker_rmi(image: &str) {
    let _ = Command::new("docker").args(["rmi", "-f", image]).output();
}

#[test]
#[serial]
fn test_docker_login_valid_credentials() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);

    assert!(docker_login(&registry, "admin", "admin"));

    docker_logout(&registry);
}

#[test]
#[serial]
fn test_docker_login_invalid_credentials() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);

    assert!(!docker_login(&registry, "invalid", "invalid"));
}

#[test]
#[serial]
fn test_docker_push_pull_image() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);
    let image_name = format!("{}/test/alpine:latest", registry);

    // Login
    assert!(docker_login(&registry, "admin", "admin"));

    // Pull a small base image
    assert!(docker_pull("alpine:latest"));

    // Tag for local registry
    assert!(docker_tag("alpine:latest", &image_name));

    // Push to local registry
    assert!(docker_push(&image_name));

    // Remove local images
    docker_rmi(&image_name);
    docker_rmi("alpine:latest");

    // Pull from local registry
    assert!(docker_pull(&image_name));

    // Cleanup
    docker_rmi(&image_name);
    docker_logout(&registry);
}

#[test]
#[serial]
fn test_docker_push_requires_authentication() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);
    let image_name = format!("{}/test/alpine:latest", registry);

    // Don't login - push should fail
    docker_pull("alpine:latest");
    docker_tag("alpine:latest", &image_name);

    assert!(!docker_push(&image_name));

    // Cleanup
    docker_rmi(&image_name);
    docker_rmi("alpine:latest");
}

#[test]
#[serial]
fn test_docker_push_with_limited_permissions() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);
    let allowed_image = format!("{}/test/allowed:latest", registry);
    let forbidden_image = format!("{}/forbidden/denied:latest", registry);

    // Login as writer (has access to test/*)
    assert!(docker_login(&registry, "writer", "writer"));

    // Pull base image
    assert!(docker_pull("alpine:latest"));

    // Push to allowed repo should succeed
    docker_tag("alpine:latest", &allowed_image);
    assert!(docker_push(&allowed_image));

    // Push to forbidden repo should fail
    docker_tag("alpine:latest", &forbidden_image);
    assert!(!docker_push(&forbidden_image));

    // Cleanup
    docker_rmi(&allowed_image);
    docker_rmi(&forbidden_image);
    docker_rmi("alpine:latest");
    docker_logout(&registry);
}

#[test]
#[serial]
fn test_docker_multi_layer_image() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);
    let image_name = format!("{}/test/nginx:latest", registry);

    // Login
    assert!(docker_login(&registry, "admin", "admin"));

    // Pull multi-layer image
    assert!(docker_pull("nginx:alpine"));

    // Tag for local registry
    assert!(docker_tag("nginx:alpine", &image_name));

    // Push to local registry
    assert!(docker_push(&image_name));

    // Remove local copy
    docker_rmi(&image_name);
    docker_rmi("nginx:alpine");

    // Pull back from local registry
    assert!(docker_pull(&image_name));

    // Cleanup
    docker_rmi(&image_name);
    docker_logout(&registry);
}

#[test]
#[serial]
fn test_docker_manifest_inspect() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);
    let image_name = format!("{}/test/alpine:latest", registry);

    // Login and push image
    assert!(docker_login(&registry, "admin", "admin"));
    assert!(docker_pull("alpine:latest"));
    assert!(docker_tag("alpine:latest", &image_name));
    assert!(docker_push(&image_name));

    // Inspect manifest
    let output = Command::new("docker")
        .args(["manifest", "inspect", &image_name])
        .output()
        .expect("Failed to run docker manifest inspect");

    assert!(output.status.success());
    let manifest_json = String::from_utf8_lossy(&output.stdout);
    assert!(manifest_json.contains("schemaVersion"));

    // Cleanup
    docker_rmi(&image_name);
    docker_rmi("alpine:latest");
    docker_logout(&registry);
}

#[test]
#[serial]
fn test_docker_concurrent_operations() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);

    // Login
    assert!(docker_login(&registry, "admin", "admin"));

    // Pull base images
    assert!(docker_pull("alpine:latest"));
    assert!(docker_pull("busybox:latest"));

    // Tag both for local registry
    let alpine_name = format!("{}/test/alpine:latest", registry);
    let busybox_name = format!("{}/test/busybox:latest", registry);

    assert!(docker_tag("alpine:latest", &alpine_name));
    assert!(docker_tag("busybox:latest", &busybox_name));

    // Push both (simulates concurrent operations)
    assert!(docker_push(&alpine_name));
    assert!(docker_push(&busybox_name));

    // Verify both can be pulled
    docker_rmi(&alpine_name);
    docker_rmi(&busybox_name);

    assert!(docker_pull(&alpine_name));
    assert!(docker_pull(&busybox_name));

    // Cleanup
    docker_rmi(&alpine_name);
    docker_rmi(&busybox_name);
    docker_rmi("alpine:latest");
    docker_rmi("busybox:latest");
    docker_logout(&registry);
}

#[test]
#[serial]
fn test_docker_reader_can_pull_only() {
    if !docker_available() {
        println!("Docker not available, skipping test");
        return;
    }

    let mut server = TestServer::new();
    server.start();

    let registry = format!("127.0.0.1:{}", server.port);
    let image_name = format!("{}/test/alpine:latest", registry);

    // First push as admin
    assert!(docker_login(&registry, "admin", "admin"));
    assert!(docker_pull("alpine:latest"));
    assert!(docker_tag("alpine:latest", &image_name));
    assert!(docker_push(&image_name));
    docker_logout(&registry);

    // Login as reader
    assert!(docker_login(&registry, "reader", "reader"));

    // Should be able to pull
    docker_rmi(&image_name);
    assert!(docker_pull(&image_name));

    // Should NOT be able to push
    let new_tag = format!("{}/test/alpine:newtag", registry);
    docker_tag(&image_name, &new_tag);
    assert!(!docker_push(&new_tag));

    // Cleanup
    docker_rmi(&image_name);
    docker_rmi(&new_tag);
    docker_rmi("alpine:latest");
    docker_logout(&registry);
}

use axum::{body::Body, extract::State, http::StatusCode, response::Response};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::state;

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub storage: StorageHealth,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageHealth {
    pub accessible: bool,
    pub blobs_path: String,
    pub manifests_path: String,
    pub writable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub checks: ReadinessChecks,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadinessChecks {
    pub storage_accessible: bool,
    pub users_loaded: bool,
}

lazy_static::lazy_static! {
    static ref START_TIME: SystemTime = SystemTime::now();
}

/// Liveness probe - is the server running?
pub async fn liveness() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"status":"alive"}"#))
        .expect("Failed to build liveness response")
}

/// Readiness probe - is the server ready to handle requests?
pub async fn readiness(State(state): State<Arc<state::App>>) -> Response {
    let storage_accessible = check_storage_accessibility();
    let users_loaded = check_users_loaded(&state).await;

    let ready = storage_accessible && users_loaded;

    let response = ReadinessResponse {
        ready,
        checks: ReadinessChecks {
            storage_accessible,
            users_loaded,
        },
    };

    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| {
                r#"{"ready":false,"checks":{"storage_accessible":false,"users_loaded":false}}"#
                    .to_string()
            }),
        ))
        .expect("Failed to build readiness response")
}

/// Detailed health endpoint
pub async fn health(State(_state): State<Arc<state::App>>) -> Response {
    let uptime = START_TIME.elapsed().map(|d| d.as_secs()).unwrap_or(0);

    let storage = StorageHealth {
        accessible: check_storage_accessibility(),
        blobs_path: "./tmp/blobs".to_string(),
        manifests_path: "./tmp/manifests".to_string(),
        writable: check_storage_writable(),
    };

    let health = HealthResponse {
        status: if storage.accessible && storage.writable {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        version: crate::utils::get_build_info().to_string(),
        uptime_seconds: uptime,
        storage,
    };

    let status = if health.status == "healthy" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string_pretty(&health).unwrap_or_else(|_| {
                r#"{"status":"unhealthy","version":"unknown","uptime_seconds":0,"storage":{"accessible":false,"blobs_path":"./tmp/blobs","manifests_path":"./tmp/manifests","writable":false}}"#
                    .to_string()
            }),
        ))
        .expect("Failed to build health response")
}

fn check_storage_accessibility() -> bool {
    Path::new("./tmp/blobs").exists() && Path::new("./tmp/manifests").exists()
}

fn check_storage_writable() -> bool {
    // Try to create a test file
    let test_file = "./tmp/.health_check";
    std::fs::write(test_file, "test").is_ok() && std::fs::remove_file(test_file).is_ok()
}

async fn check_users_loaded(state: &Arc<state::App>) -> bool {
    let users = state.users.lock().await;
    !users.is_empty()
}

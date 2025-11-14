use serde_json::{json, Value};
use std::sync::Arc;

use crate::{state, utils};
use axum::{
    extract::{Path, State},
    response::Json,
};

pub(crate) async fn index(State(data): State<Arc<state::App>>) -> Json<Value> {
    let status = data.server_status.lock().await;
    log::info!("meta/index: server_status: {}", status);
    return Json(json!({
        "server": format!("grain {} status {}", utils::get_build_info(), status)
    }));
}

pub(crate) async fn catch_all_head(Path(path): Path<String>) -> String {
    log::error!("meta/catch_all: HEAD {}", path);
    return "Not found".to_string();
}

pub(crate) async fn catch_all_get(Path(path): Path<String>) -> String {
    log::error!("meta/catch_all: GET {}", path);
    return "Not found".to_string();
}

pub(crate) async fn catch_all_post(Path(path): Path<String>) -> String {
    log::error!("meta/catch_all: POST {}", path);
    return "Not found".to_string();
}

pub(crate) async fn catch_all_put(Path(path): Path<String>) -> String {
    log::error!("meta/catch_all: PUT {}", path);
    return "Not found".to_string();
}

pub(crate) async fn catch_all_patch(Path(path): Path<String>) -> String {
    log::error!("meta/catch_all: PATCH {}", path);
    return "Not found".to_string();
}

pub(crate) async fn catch_all_delete(Path(path): Path<String>) -> String {
    log::error!("meta/catch_all: DELETE {}", path);
    return "Not found".to_string();
}

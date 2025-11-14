// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
// | end-3  | `GET` / `HEAD` | `/v2/<name>/manifests/<reference>`                           | `200`       | `404`             |
// | end-7  | `PUT`          | `/v2/<name>/manifests/<reference>`                           | `201`       | `404`             |
// | end-9  | `DELETE`       | `/v2/<name>/manifests/<reference>`                           | `202`       | `404`/`400`/`405` |

use serde_json::{json, Value};
use std::sync::Arc;

use crate::{
    response::{not_found, not_implemented},
    state,
    storage::write_manifest,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::Request,
    response::{Json, Response},
};

// end-3 GET /v2/:name/manifests/:reference
pub(crate) async fn get_manifest_by_reference(
    State(data): State<Arc<state::App>>,
    Path((org, repo, reference)): Path<(String, String, String)>,
) -> Json<Value> {
    let status = data.server_status.lock().await;
    log::info!(
        "manifests/get_manifest_by_reference: org: {}, repo: {}, reference: {}",
        org,
        repo,
        reference
    );
    Json(json!({
        "not_implemented": format!("org {} repo {} reference {} server_status {}", org, repo, reference, status)
    }))
}

// end-3 HEAD /v2/:name/manifests/:reference
pub(crate) async fn head_manifest_by_reference(
    Path((org, repo, reference)): Path<(String, String, String)>,
) -> Response<String> {
    log::info!(
        "manifests/head_manifest_by_reference: org: {}, repo: {}, reference: {}",
        org,
        repo,
        reference
    );

    not_found()
}

// end-7 PUT /v2/:name/manifests/:reference
#[axum::debug_handler]
pub(crate) async fn put_manifest_by_reference(
    Path((org, repo, reference)): Path<(String, String, String)>,
    body: Request<Body>,
) -> Response<String> {
    log::info!(
        "manifests/put_manifest_by_reference: org: {}, repo: {}, reference: {}",
        org,
        repo,
        reference
    );

    let success = write_manifest(&org, &repo, &reference, body.into_body()).await;
    if !success {
        return Response::builder()
            .status(400)
            .body("400 Bad Request".to_string())
            .expect("Failed to build response");
    }

    Response::builder()
        .status(201)
        .header(
            "Location",
            format!("/v2/{}/{}/manifests/{}", org, repo, reference),
        )
        .body("201 Created".to_string())
        .expect("Failed to build response")
}

// end-9 DELETE /v2/:name/manifests/:reference
pub(crate) async fn delete_manifest_by_reference(
    Path(name): Path<String>,
    Path(reference): Path<String>,
) -> Response<String> {
    log::info!(
        "manifests/delete_manifest_by_reference: name: {}, reference: {}",
        name,
        reference
    );
    not_implemented()
}

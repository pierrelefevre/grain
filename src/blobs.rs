// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
// | end-1  | `GET`          | `/v2/`                                                       | `200`       | `404`/`401`       |
// | end-2  | `GET` / `HEAD` | `/v2/<name>/blobs/<digest>`                                  | `200`       | `404`             |
// | end-4a | `POST`         | `/v2/<name>/blobs/uploads/`                                  | `202`       | `404`             |
// | end-4b | `POST`         | `/v2/<name>/blobs/uploads/?digest=<digest>`                  | `201`/`202` | `404`/`400`       |
// | end-5  | `PATCH`        | `/v2/<name>/blobs/uploads/<reference>`                       | `202`       | `404`/`416`       |
// | end-6  | `PUT`          | `/v2/<name>/blobs/uploads/<reference>?digest=<digest>`       | `201`       | `404`/`400`       |
// | end-10 | `DELETE`       | `/v2/<name>/blobs/<digest>`                                  | `202`       | `404`/`405`       |
// | end-11 | `POST`         | `/v2/<name>/blobs/uploads/?mount=<digest>&from=<other_name>` | `201`       | `404`             |

use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::{
    auth, state,
    storage::{self, write_blob},
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, Request, StatusCode},
    response::{Json, Response},
};

// end-2 GET /v2/:name/blobs/:digest
pub(crate) async fn get_blob_by_digest(
    State(state): State<Arc<state::App>>,
    Path((org, repo, digest_string)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response<Body> {
    log::info!(
        "blobs/get_blob_by_digest: org: {}, repo {}, digest: {}",
        org,
        repo,
        digest_string
    );

    let host = &state.args.host;

    // Authenticate
    if auth::get(State(state.clone()), headers).await.status() != StatusCode::OK {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                "WWW-Authenticate",
                format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
            )
            .body(Body::from("401 Unauthorized"))
            .unwrap();
    }

    // Strip sha256: prefix if present
    let clean_digest = digest_string
        .strip_prefix("sha256:")
        .unwrap_or(&digest_string);

    // Read blob from storage
    match storage::read_blob(&org, &repo, clean_digest) {
        Ok(blob_data) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Length", blob_data.len().to_string())
            .header("Docker-Content-Digest", format!("sha256:{}", clean_digest))
            .header("Content-Type", "application/octet-stream")
            .body(Body::from(blob_data))
            .unwrap(),
        Err(e) => {
            log::warn!(
                "blobs/get_blob_by_digest: blob not found: {}/{}/{}: {}",
                org,
                repo,
                clean_digest,
                e
            );
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))
                .unwrap()
        }
    }
}

// end-2 HEAD /v2/:name/blobs/:digest
pub(crate) async fn head_blob_by_digest(
    State(state): State<Arc<state::App>>,
    Path((org, repo, digest_string)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response<Body> {
    log::info!(
        "blobs/head_blob_by_digest: org: {}, repo {}, digest: {}",
        org,
        repo,
        digest_string
    );

    let host = &state.args.host;

    // Authenticate
    if auth::get(State(state.clone()), headers).await.status() != StatusCode::OK {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                "WWW-Authenticate",
                format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
            )
            .body(Body::empty())
            .unwrap();
    }

    // Strip sha256: prefix if present
    let clean_digest = digest_string
        .strip_prefix("sha256:")
        .unwrap_or(&digest_string);

    // Check if blob exists and get metadata
    match storage::blob_metadata(&org, &repo, clean_digest) {
        Ok(metadata) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Length", metadata.len().to_string())
            .header("Docker-Content-Digest", format!("sha256:{}", clean_digest))
            .header("Content-Type", "application/octet-stream")
            .body(Body::empty())
            .unwrap(),
        Err(e) => {
            log::warn!(
                "blobs/head_blob_by_digest: blob not found: {}/{}/{}: {}",
                org,
                repo,
                clean_digest,
                e
            );
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap()
        }
    }
}

// end-4a POST /v2/:name/blobs/uploads/
// end-4b POST /v2/:name/blobs/uploads/?digest=:digest
// end-11 POST /v2/:name/blobs/uploads/?mount=:digest&from=:other_name
#[derive(Deserialize)]
pub(crate) struct PostBlobUploadQueryParams {
    digest: Option<String>,
    mount: Option<String>,
    from: Option<String>,
}

pub(crate) async fn post_blob_upload(
    Path((org, repo)): Path<(String, String)>,
) -> Response<String> {
    log::info!("blobs/post_blob_upload: org: {}, repo: {}", org, repo,);

    Response::builder()
        .status(202)
        .header("Location", format!("/v2/{}/{}/blobs/uploads/", org, repo))
        .body("202 Accepted".to_string())
        .expect("Failed to build response")
}

pub(crate) async fn put_blob_upload(
    Path((org, repo)): Path<(String, String)>,
    query: Query<PostBlobUploadQueryParams>,
    body: Request<Body>,
) -> Response<String> {
    log::info!(
        "blobs/put_blob_upload: org: {}, repo: {}, digest: {:#?}, mount: {:#?}, from: {:#?}",
        org,
        repo,
        query.digest,
        query.mount,
        query.from
    );

    let success = match &query.digest {
        Some(digest) => write_blob(&org, &repo, digest, body.into_body()).await,
        None => false,
    };

    if !success {
        return Response::builder()
            .status(400)
            .body("400 Bad Request".to_string())
            .expect("Failed to build response");
    }

    Response::builder()
        .status(201)
        .header("Location", format!("/v2/{}/blobs/uploads/{}", org, repo))
        .header(
            "Docker-Content-Digest",
            query.digest.as_ref().expect("Digest should exist"),
        )
        .body("201 Created".to_string())
        .expect("Failed to build response")
}

// end-5 PATCH /v2/:name/blobs/uploads/:reference
pub(crate) async fn patch_blob_upload(
    State(data): State<Arc<state::App>>,
    Path(name): Path<String>,
    Path(reference): Path<String>,
) -> Json<Value> {
    let status = data.server_status.lock().await;
    log::info!(
        "blobs/patch_blob_upload: name: {}, reference: {}",
        name,
        reference
    );
    Json(json!({
        "not_implemented": format!("name {} reference {} server_status {}", name, reference, status)
    }))
}

// end-6 PUT /v2/:name/blobs/uploads/:reference?digest=:digest
#[derive(Deserialize)]
pub(crate) struct End6QueryParams {
    digest: String,
}
pub(crate) async fn put_blob_upload_by_reference(
    State(data): State<Arc<state::App>>,
    Path(name): Path<String>,
    Path(reference): Path<String>,
    query: Query<End6QueryParams>,
) -> Json<Value> {
    let status = data.server_status.lock().await;
    log::info!(
        "blobs/put_blob_upload_by_reference: name: {}, reference: {}, digest: {}",
        name,
        reference,
        query.digest
    );
    Json(json!({
        "not_implemented": format!("name {} reference {} digest {} server_status {}", name, reference, query.digest, status)
    }))
}

// end-10 DELETE /v2/:name/blobs/:digest
pub(crate) async fn delete_blob_by_digest(
    State(data): State<Arc<state::App>>,
    Path(name): Path<String>,
    Path(digest): Path<String>,
) -> Json<Value> {
    let status = data.server_status.lock().await;
    log::info!(
        "blobs/delete_blob_by_digest: name: {}, digest: {}",
        name,
        digest
    );
    Json(json!({
        "not_implemented": format!("name {} digest {} server_status {}", name, digest, status)
    }))
}

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
    auth, response, state,
    storage::{self, write_blob},
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Json, Response},
};
use bytes::Bytes;

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
    #[allow(dead_code)]
    mount: Option<String>,
    #[allow(dead_code)]
    from: Option<String>,
}

pub(crate) async fn post_blob_upload(
    State(state): State<Arc<state::App>>,
    Path((org, repo)): Path<(String, String)>,
    Query(params): Query<PostBlobUploadQueryParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    log::info!("blobs/post_blob_upload: org: {}, repo: {}", org, repo);

    let host = &state.args.host;

    if auth::get(State(state.clone()), headers.clone())
        .await
        .status()
        != StatusCode::OK
    {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                "WWW-Authenticate",
                format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
            )
            .body(Body::from("401 Unauthorized"))
            .unwrap();
    }

    // If digest is provided, handle monolithic upload (end-4b)
    if let Some(digest_string) = params.digest {
        let success = write_blob(&org, &repo, &digest_string, Body::from(body)).await;

        if !success {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Digest mismatch or write failed"))
                .unwrap();
        }

        let clean_digest = digest_string
            .strip_prefix("sha256:")
            .unwrap_or(&digest_string);

        return Response::builder()
            .status(StatusCode::CREATED)
            .header(
                "Location",
                format!(
                    "http://{}/v2/{}/{}/blobs/sha256:{}",
                    host, org, repo, clean_digest
                ),
            )
            .header("Docker-Content-Digest", format!("sha256:{}", clean_digest))
            .body(Body::empty())
            .unwrap();
    }

    // Create new upload session (end-4a)
    let uuid = uuid::Uuid::new_v4().to_string();

    if let Err(e) = storage::init_upload_session(&org, &repo, &uuid) {
        log::error!("Failed to init upload session: {}", e);
        return response::internal_error();
    }

    let location = format!("http://{}/v2/{}/{}/blobs/uploads/{}", host, org, repo, uuid);

    Response::builder()
        .status(StatusCode::ACCEPTED)
        .header("Location", location)
        .header("Range", "0-0")
        .header("Docker-Upload-UUID", uuid)
        .body(Body::empty())
        .unwrap()
}

// end-5 PATCH /v2/:name/blobs/uploads/:reference
pub(crate) async fn patch_blob_upload(
    State(state): State<Arc<state::App>>,
    Path((org, repo, uuid)): Path<(String, String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    log::info!(
        "blobs/patch_blob_upload: org: {}, repo: {}, uuid: {}",
        org,
        repo,
        uuid
    );

    let host = &state.args.host;

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

    match storage::append_upload_chunk(&org, &repo, &uuid, &body) {
        Ok(total_size) => {
            let location = format!("http://{}/v2/{}/{}/blobs/uploads/{}", host, org, repo, uuid);

            Response::builder()
                .status(StatusCode::ACCEPTED)
                .header("Location", location)
                .header("Range", format!("0-{}", total_size.saturating_sub(1)))
                .header("Docker-Upload-UUID", &uuid)
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            log::error!("Failed to append chunk for upload {}: {}", uuid, e);
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Upload session not found"))
                .unwrap()
        }
    }
}

// end-6 PUT /v2/:name/blobs/uploads/:reference?digest=:digest
#[derive(Deserialize)]
pub(crate) struct End6QueryParams {
    digest: String,
}

pub(crate) async fn put_blob_upload_by_reference(
    State(state): State<Arc<state::App>>,
    Path((org, repo, uuid)): Path<(String, String, String)>,
    Query(params): Query<End6QueryParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    log::info!(
        "blobs/put_blob_upload_by_reference: org: {}, repo: {}, uuid: {}, digest: {}",
        org,
        repo,
        uuid,
        params.digest
    );

    let host = &state.args.host;

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

    // Append final chunk if body is not empty
    if !body.is_empty() {
        if let Err(e) = storage::append_upload_chunk(&org, &repo, &uuid, &body) {
            log::error!("Failed to append final chunk: {}", e);
            return response::internal_error();
        }
    }

    // Finalize upload and validate digest
    match storage::finalize_upload(&org, &repo, &uuid, &params.digest) {
        Ok(actual_digest) => {
            let location = format!(
                "http://{}/v2/{}/{}/blobs/sha256:{}",
                host, org, repo, actual_digest
            );

            Response::builder()
                .status(StatusCode::CREATED)
                .header("Location", location)
                .header("Docker-Content-Digest", format!("sha256:{}", actual_digest))
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            log::error!("Failed to finalize upload: {}", e);

            // Clean up failed upload
            let _ = storage::delete_upload_session(&org, &repo, &uuid);

            if e.contains("Digest mismatch") {
                response::digest_mismatch()
            } else {
                response::internal_error()
            }
        }
    }
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

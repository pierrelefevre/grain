// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
// | end-3  | `GET` / `HEAD` | `/v2/<name>/manifests/<reference>`                           | `200`       | `404`             |
// | end-7  | `PUT`          | `/v2/<name>/manifests/<reference>`                           | `201`       | `404`             |
// | end-9  | `DELETE`       | `/v2/<name>/manifests/<reference>`                           | `202`       | `404`/`400`/`405` |

use serde_json::Value;
use std::sync::Arc;

use crate::{auth, permissions, state, storage};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode},
    response::Response,
};

fn detect_manifest_content_type(manifest_data: &[u8]) -> String {
    if let Ok(json_str) = std::str::from_utf8(manifest_data) {
        if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
            if let Some(media_type) = parsed.get("mediaType").and_then(|v| v.as_str()) {
                return media_type.to_string();
            }
        }
    }
    "application/vnd.oci.image.manifest.v1+json".to_string()
}

// end-3 GET /v2/:name/manifests/:reference
pub(crate) async fn get_manifest_by_reference(
    State(state): State<Arc<state::App>>,
    Path((org, repo, reference)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response<Body> {
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

    let clean_reference = reference.strip_prefix("sha256:").unwrap_or(&reference);

    log::info!(
        "manifests/get_manifest_by_reference: org: {}, repo: {}, reference: {}",
        org,
        repo,
        clean_reference
    );

    match storage::read_manifest(&org, &repo, clean_reference) {
        Ok(manifest_data) => {
            let digest = sha256::digest(&manifest_data);
            let content_type = detect_manifest_content_type(&manifest_data);

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Length", manifest_data.len().to_string())
                .header("Content-Type", content_type)
                .header("Docker-Content-Digest", format!("sha256:{}", digest))
                .body(Body::from(manifest_data))
                .unwrap()
        }
        Err(e) => {
            log::error!(
                "Failed to read manifest {}/{}/{}: {}",
                org,
                repo,
                clean_reference,
                e
            );
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))
                .unwrap()
        }
    }
}

// end-3 HEAD /v2/:name/manifests/:reference
pub(crate) async fn head_manifest_by_reference(
    State(state): State<Arc<state::App>>,
    Path((org, repo, reference)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response<Body> {
    let host = &state.args.host;
    let repository = format!("{}/{}", org, repo);
    let clean_reference = reference.strip_prefix("sha256:").unwrap_or(&reference);

    // Check permission (Pull for manifest retrieval, tag-specific)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        Some(clean_reference),
        permissions::Action::Pull,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::from("403 Forbidden: Insufficient permissions"))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(
                        "WWW-Authenticate",
                        format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
                    )
                    .body(Body::from("401 Unauthorized"))
                    .unwrap()
            };
        }
    }

    log::info!(
        "manifests/head_manifest_by_reference: org: {}, repo: {}, reference: {}",
        org,
        repo,
        clean_reference
    );

    if !storage::manifest_exists(&org, &repo, clean_reference) {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .unwrap();
    }

    match storage::read_manifest(&org, &repo, clean_reference) {
        Ok(manifest_data) => {
            let digest = sha256::digest(&manifest_data);
            let content_type = detect_manifest_content_type(&manifest_data);

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Length", manifest_data.len().to_string())
                .header("Content-Type", content_type)
                .header("Docker-Content-Digest", format!("sha256:{}", digest))
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            log::error!(
                "Failed to read manifest {}/{}/{}: {}",
                org,
                repo,
                clean_reference,
                e
            );
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))
                .unwrap()
        }
    }
}

// end-7 PUT /v2/:name/manifests/:reference
#[axum::debug_handler]
pub(crate) async fn put_manifest_by_reference(
    State(state): State<Arc<state::App>>,
    Path((org, repo, reference)): Path<(String, String, String)>,
    headers: HeaderMap,
    body: Request<Body>,
) -> Response {
    log::info!(
        "manifests/put_manifest_by_reference: org: {}, repo: {}, reference: {}",
        org,
        repo,
        reference
    );

    let host = &state.args.host;
    let repository = format!("{}/{}", org, repo);
    let clean_reference = reference.strip_prefix("sha256:").unwrap_or(&reference);

    // Check permission (Push for manifest upload, tag-specific)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        Some(clean_reference),
        permissions::Action::Push,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::from("403 Forbidden: Insufficient permissions"))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(
                        "WWW-Authenticate",
                        format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
                    )
                    .body(Body::from("401 Unauthorized"))
                    .unwrap()
            };
        }
    }

    let success = storage::write_manifest(&org, &repo, &reference, body.into_body()).await;
    if !success {
        return Response::builder()
            .status(400)
            .body(Body::from("400 Bad Request"))
            .expect("Failed to build response");
    }

    Response::builder()
        .status(201)
        .header(
            "Location",
            format!("/v2/{}/{}/manifests/{}", org, repo, reference),
        )
        .body(Body::empty())
        .expect("Failed to build response")
}

// end-9 DELETE /v2/:name/manifests/:reference
pub(crate) async fn delete_manifest_by_reference(
    State(state): State<Arc<state::App>>,
    Path((org, repo, reference)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response<Body> {
    let host = &state.args.host;
    let repository = format!("{}/{}", org, repo);
    let clean_reference = reference.strip_prefix("sha256:").unwrap_or(&reference);

    // Check permission (Delete for manifest deletion, tag-specific)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        Some(clean_reference),
        permissions::Action::Delete,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::from("403 Forbidden: Insufficient permissions"))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(
                        "WWW-Authenticate",
                        format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
                    )
                    .body(Body::from("401 Unauthorized"))
                    .unwrap()
            };
        }
    }

    log::info!(
        "manifests/delete_manifest_by_reference: org: {}, repo: {}, reference: {}",
        org,
        repo,
        clean_reference
    );

    // Delete manifest
    match storage::delete_manifest(&org, &repo, clean_reference) {
        Ok(()) => {
            log::info!("Deleted manifest {}/{}/{}", org, repo, clean_reference);

            Response::builder()
                .status(StatusCode::ACCEPTED)
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                log::warn!(
                    "Attempted to delete non-existent manifest {}/{}/{}",
                    org,
                    repo,
                    clean_reference
                );
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("404 Not Found"))
                    .unwrap()
            } else {
                log::error!(
                    "Failed to delete manifest {}/{}/{}: {}",
                    org,
                    repo,
                    clean_reference,
                    e
                );
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal server error"))
                    .unwrap()
            }
        }
    }
}

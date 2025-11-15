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
use std::sync::Arc;

use crate::{
    auth, permissions, response, state,
    storage::{self, write_blob},
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
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
    let repository = format!("{}/{}", org, repo);

    // Check permission (Pull for blob retrieval)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        None,
        permissions::Action::Pull,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                response::forbidden()
            } else {
                response::unauthorized(host)
            };
        }
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
            response::blob_unknown(&format!("sha256:{}", clean_digest))
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
    let repository = format!("{}/{}", org, repo);

    // Check permission (Pull for blob retrieval)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        None,
        permissions::Action::Pull,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::empty())
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(
                        "WWW-Authenticate",
                        format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
                    )
                    .body(Body::empty())
                    .unwrap()
            };
        }
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
            response::blob_unknown(&format!("sha256:{}", clean_digest))
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
    State(state): State<Arc<state::App>>,
    Path((org, repo)): Path<(String, String)>,
    Query(params): Query<PostBlobUploadQueryParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    log::info!("blobs/post_blob_upload: org: {}, repo: {}", org, repo);

    let host = &state.args.host;
    let repository = format!("{}/{}", org, repo);

    // Check permission (Push for blob upload)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        None,
        permissions::Action::Push,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                response::forbidden()
            } else {
                response::unauthorized(host)
            };
        }
    }

    // Handle blob mounting (end-11)
    if let (Some(mount_digest), Some(from_repo)) = (&params.mount, &params.from) {
        let clean_digest = mount_digest.strip_prefix("sha256:").unwrap_or(mount_digest);

        // Parse source repository (format: "org/repo")
        let from_parts: Vec<&str> = from_repo.split('/').collect();
        if from_parts.len() == 2 {
            let source_org = from_parts[0];
            let source_repo = from_parts[1];
            let source_repository = format!("{}/{}", source_org, source_repo);

            // Check if user has pull permission on source repository
            if auth::check_permission(
                &state,
                &headers,
                &source_repository,
                None,
                permissions::Action::Pull,
            )
            .await
            .is_ok()
            {
                // Attempt to mount blob
                match storage::mount_blob(source_org, source_repo, &org, &repo, clean_digest) {
                    Ok(()) => {
                        log::info!(
                            "Mounted blob {} from {} to {}",
                            clean_digest,
                            from_repo,
                            repository
                        );

                        let location = format!(
                            "http://{}/v2/{}/{}/blobs/sha256:{}",
                            host, org, repo, clean_digest
                        );

                        return Response::builder()
                            .status(StatusCode::CREATED)
                            .header("Location", location)
                            .header("Docker-Content-Digest", format!("sha256:{}", clean_digest))
                            .body(Body::empty())
                            .unwrap();
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to mount blob {}: {} - falling back to upload",
                            clean_digest,
                            e
                        );
                        // Fall through to regular upload session creation
                    }
                }
            } else {
                log::warn!("User lacks permission to mount from {}", from_repo);
                // Fall through to regular upload
            }
        }
    }

    // If digest is provided, handle monolithic upload (end-4b)
    if let Some(digest_string) = params.digest {
        let success = write_blob(&org, &repo, &digest_string, Body::from(body)).await;

        if !success {
            return response::digest_invalid(&digest_string);
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
    let repository = format!("{}/{}", org, repo);

    // Check permission (Push for blob upload)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        None,
        permissions::Action::Push,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                response::forbidden()
            } else {
                response::unauthorized(host)
            };
        }
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
            response::blob_upload_unknown(&uuid)
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
    let repository = format!("{}/{}", org, repo);

    // Check permission (Push for blob upload)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        None,
        permissions::Action::Push,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                response::forbidden()
            } else {
                response::unauthorized(host)
            };
        }
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
                response::digest_invalid(&params.digest)
            } else {
                response::internal_error()
            }
        }
    }
}

// end-10 DELETE /v2/:name/blobs/:digest
pub(crate) async fn delete_blob_by_digest(
    State(state): State<Arc<state::App>>,
    Path((org, repo, digest_string)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response<Body> {
    let host = &state.args.host;
    let repository = format!("{}/{}", org, repo);

    // Check permission (Delete for blob deletion)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        None,
        permissions::Action::Delete,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                response::forbidden()
            } else {
                response::unauthorized(host)
            };
        }
    }

    // Clean digest (strip sha256: prefix if present)
    let clean_digest = digest_string
        .strip_prefix("sha256:")
        .unwrap_or(&digest_string);

    log::info!(
        "blobs/delete_blob_by_digest: org: {}, repo: {}, digest: {}",
        org,
        repo,
        clean_digest
    );

    // Delete blob
    match storage::delete_blob(&org, &repo, clean_digest) {
        Ok(()) => {
            log::info!("Deleted blob {}/{}/{}", org, repo, clean_digest);

            Response::builder()
                .status(StatusCode::ACCEPTED)
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                log::warn!(
                    "Attempted to delete non-existent blob {}/{}/{}",
                    org,
                    repo,
                    clean_digest
                );
                response::blob_unknown(&format!("sha256:{}", clean_digest))
            } else {
                log::error!(
                    "Failed to delete blob {}/{}/{}: {}",
                    org,
                    repo,
                    clean_digest,
                    e
                );
                response::internal_error()
            }
        }
    }
}

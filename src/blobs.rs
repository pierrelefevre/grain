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
    response::{not_found, not_implemented},
    state,
    storage::write_blob,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::Request,
    response::{Json, Response},
};

// end-2 GET /v2/:name/blobs/:digest
pub(crate) async fn get_blob_by_digest(
    Path((org, repo, digest)): Path<(String, String, String)>,
) -> Response<String> {
    log::info!(
        "blobs/get_blob_by_digest: org: {}, repo {}, digest: {}",
        org,
        repo,
        digest
    );

    return not_implemented();
}

// end-2 HEAD /v2/:name/blobs/:digest
pub(crate) async fn head_blob_by_digest(
    Path((org, repo, digest)): Path<(String, String, String)>,
) -> Response<String> {
    log::info!(
        "blobs/head_blob_by_digest: org: {}, repo {}, digest: {}",
        org,
        repo,
        digest
    );

    return not_found();
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

    return Response::builder()
        .status(202)
        .header("Location", format!("/v2/{}/{}/blobs/uploads/", org, repo))
        .body("202 Accepted".to_string())
        .unwrap();
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
        Some(digest) => write_blob(&org, &repo, &digest, body.into_body()).await,
        None => false,
    };

    if !success {
        return Response::builder()
            .status(400)
            .body("400 Bad Request".to_string())
            .unwrap();
    }

    return Response::builder()
        .status(201)
        .header("Location", format!("/v2/{}/blobs/uploads/{}", org, repo))
        .header("Docker-Content-Digest", query.digest.as_ref().unwrap())
        .body("201 Created".to_string())
        .unwrap();
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
    return Json(json!({
        "not_implemented": format!("name {} reference {} server_status {}", name, reference, status)
    }));
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
    return Json(json!({
        "not_implemented": format!("name {} reference {} digest {} server_status {}", name, reference, query.digest, status)
    }));
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
    return Json(json!({
        "not_implemented": format!("name {} digest {} server_status {}", name, digest, status)
    }));
}

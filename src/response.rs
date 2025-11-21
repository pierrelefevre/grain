use crate::errors::{ErrorCode, OciErrorResponse};
use axum::{body::Body, http::Response, http::StatusCode, response::IntoResponse};

pub(crate) fn unauthorized(host: &str) -> Response<Body> {
    let error = OciErrorResponse::new(ErrorCode::Unauthorized, "authentication required");

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(
            "WWW-Authenticate",
            format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
        )
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&error).unwrap_or_else(
            |_| {
                r#"{"errors":[{"code":"UNAUTHORIZED","message":"authentication required"}]}"#
                    .to_string()
            },
        )))
        .expect("Failed to build unauthorized response")
}

pub(crate) fn forbidden() -> Response<Body> {
    OciErrorResponse::new(ErrorCode::Denied, "access denied: insufficient permissions")
        .into_response()
}

pub(crate) fn not_found() -> Response<Body> {
    OciErrorResponse::new(ErrorCode::BlobUnknown, "resource not found").into_response()
}

pub(crate) fn blob_unknown(digest: &str) -> Response<Body> {
    OciErrorResponse::with_detail(
        ErrorCode::BlobUnknown,
        "blob unknown to registry",
        format!("digest: {}", digest),
    )
    .into_response()
}

pub(crate) fn manifest_unknown(reference: &str) -> Response<Body> {
    OciErrorResponse::with_detail(
        ErrorCode::ManifestUnknown,
        "manifest unknown to registry",
        format!("reference: {}", reference),
    )
    .into_response()
}

pub(crate) fn digest_invalid(digest: &str) -> Response<Body> {
    OciErrorResponse::with_detail(
        ErrorCode::DigestInvalid,
        "provided digest did not match uploaded content",
        format!("digest: {}", digest),
    )
    .into_response()
}

pub(crate) fn manifest_invalid(reason: &str) -> Response<Body> {
    OciErrorResponse::with_detail(ErrorCode::ManifestInvalid, "manifest invalid", reason)
        .into_response()
}

#[allow(dead_code)]
pub(crate) fn name_invalid(name: &str) -> Response<Body> {
    OciErrorResponse::with_detail(ErrorCode::NameInvalid, "invalid repository name", name)
        .into_response()
}

pub(crate) fn blob_upload_unknown(uuid: &str) -> Response<Body> {
    OciErrorResponse::with_detail(
        ErrorCode::BlobUploadUnknown,
        "upload session not found",
        format!("uuid: {}", uuid),
    )
    .into_response()
}

pub(crate) fn internal_error() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"errors":[{"code":"UNKNOWN","message":"internal server error"}]}"#,
        ))
        .expect("Failed to build internal error response")
}

pub(crate) fn conflict(message: &str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::CONFLICT)
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"errors":[{{"code":"UNSUPPORTED","message":"{}"}}]}}"#,
            message
        )))
        .expect("Failed to build conflict response")
}

use axum::{body::Body, http::Response};

pub(crate) fn unauthorized(host: &str) -> Response<String> {
    Response::builder()
        .status(401)
        .header(
            "WWW-Authenticate",
            format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
        )
        .body("401 Unauthorized".to_string())
        .unwrap()
}

pub(crate) fn ok() -> Response<String> {
    Response::builder()
        .status(200)
        .body("200 OK".to_string())
        .unwrap()
}

#[allow(dead_code)]
pub(crate) fn bad_request(message: &str) -> Response<Body> {
    Response::builder()
        .status(400)
        .body(Body::from(message.to_string()))
        .unwrap()
}

pub(crate) fn internal_error() -> Response<Body> {
    Response::builder()
        .status(500)
        .body(Body::from("Internal server error"))
        .unwrap()
}

pub(crate) fn digest_mismatch() -> Response<Body> {
    Response::builder()
        .status(400)
        .body(Body::from("Digest mismatch"))
        .unwrap()
}

pub(crate) fn forbidden() -> Response<Body> {
    Response::builder()
        .status(403)
        .body(Body::from("403 Forbidden: Insufficient permissions"))
        .unwrap()
}

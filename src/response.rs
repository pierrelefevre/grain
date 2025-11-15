use axum::{body::Body, http::Response};

pub(crate) fn unauthorized(host: &str) -> Response<Body> {
    Response::builder()
        .status(401)
        .header(
            "WWW-Authenticate",
            format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
        )
        .body(Body::from("401 Unauthorized"))
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

pub(crate) fn not_found() -> Response<Body> {
    Response::builder()
        .status(404)
        .body(Body::from("404 Not Found"))
        .unwrap()
}

pub(crate) fn no_content() -> Response<Body> {
    Response::builder().status(204).body(Body::empty()).unwrap()
}

pub(crate) fn conflict(message: &str) -> Response<Body> {
    Response::builder()
        .status(409)
        .body(Body::from(message.to_string()))
        .unwrap()
}

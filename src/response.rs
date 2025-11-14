use axum::http::Response;

pub(crate) fn not_found() -> Response<String> {
    Response::builder()
        .status(404)
        .body("404 Not Found".to_string())
        .unwrap()
}

pub(crate) fn not_implemented() -> Response<String> {
    Response::builder()
        .status(501)
        .body("501 Not Implemented".to_string())
        .unwrap()
}

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
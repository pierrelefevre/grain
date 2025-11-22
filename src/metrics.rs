use axum::{body::Body, http::StatusCode, response::Response};
use prometheus::{
    register_histogram_vec, register_int_counter, register_int_counter_vec, Encoder, HistogramVec,
    IntCounter, IntCounterVec, TextEncoder,
};

lazy_static::lazy_static! {
    // Prometheus metric registration - intentionally panics on failure as these are
    // initialization-time requirements. If metrics cannot be registered, the server
    // should fail early rather than run with broken metrics.

    // Request counters
    pub static ref HTTP_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "grain_http_requests_total",
        "Total number of HTTP requests",
        &["method", "endpoint", "status"]
    ).unwrap();

    pub static ref BLOB_UPLOADS_TOTAL: IntCounter = register_int_counter!(
        "grain_blob_uploads_total",
        "Total number of blob uploads"
    ).unwrap();

    pub static ref BLOB_DOWNLOADS_TOTAL: IntCounter = register_int_counter!(
        "grain_blob_downloads_total",
        "Total number of blob downloads"
    ).unwrap();

    pub static ref MANIFEST_UPLOADS_TOTAL: IntCounter = register_int_counter!(
        "grain_manifest_uploads_total",
        "Total number of manifest uploads"
    ).unwrap();

    pub static ref MANIFEST_DOWNLOADS_TOTAL: IntCounter = register_int_counter!(
        "grain_manifest_downloads_total",
        "Total number of manifest downloads"
    ).unwrap();

    pub static ref AUTH_FAILURES_TOTAL: IntCounter = register_int_counter!(
        "grain_auth_failures_total",
        "Total number of authentication failures"
    ).unwrap();

    pub static ref PERMISSION_DENIALS_TOTAL: IntCounter = register_int_counter!(
        "grain_permission_denials_total",
        "Total number of permission denials"
    ).unwrap();

    // Latency histograms
    pub static ref REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "grain_request_duration_seconds",
        "HTTP request duration in seconds",
        &["method", "endpoint"]
    ).unwrap();
}

/// Prometheus metrics endpoint
pub async fn metrics() -> Response {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        log::error!("Failed to encode metrics: {}", e);
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Failed to encode metrics"))
            .expect("Failed to build metrics error response");
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(
            "Content-Type",
            format!("{}; charset=utf-8", encoder.format_type()),
        )
        .body(Body::from(buffer))
        .expect("Failed to build metrics response")
}

use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;

use crate::metrics;

pub async fn track_metrics(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    // Process request
    let response = next.run(req).await;

    // Record metrics
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    // Normalize endpoint for metrics (avoid cardinality explosion)
    let endpoint = normalize_endpoint(&path);

    metrics::HTTP_REQUESTS_TOTAL
        .with_label_values(&[&method, &endpoint, &status])
        .inc();

    metrics::REQUEST_DURATION
        .with_label_values(&[&method, &endpoint])
        .observe(duration);

    response
}

fn normalize_endpoint(path: &str) -> String {
    // Replace dynamic segments with placeholders
    if path == "/v2/" {
        return "/v2/".to_string();
    }
    if path.starts_with("/v2/") {
        if path.contains("/blobs/") {
            if path.contains("/uploads/") {
                return "/v2/{name}/blobs/uploads/{reference}".to_string();
            }
            return "/v2/{name}/blobs/{digest}".to_string();
        } else if path.contains("/manifests/") {
            return "/v2/{name}/manifests/{reference}".to_string();
        } else if path.contains("/tags/") {
            return "/v2/{name}/tags/list".to_string();
        }
    }
    if path.starts_with("/admin/") {
        if path.contains("/users/") && path.split('/').count() > 3 {
            if path.contains("/permissions") {
                return "/admin/users/{username}/permissions".to_string();
            }
            return "/admin/users/{username}".to_string();
        }
        return path.to_string();
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_endpoint() {
        assert_eq!(
            normalize_endpoint("/v2/myorg/myrepo/blobs/sha256:abc123"),
            "/v2/{name}/blobs/{digest}"
        );
        assert_eq!(
            normalize_endpoint("/v2/myorg/myrepo/manifests/latest"),
            "/v2/{name}/manifests/{reference}"
        );
        assert_eq!(
            normalize_endpoint("/v2/myorg/myrepo/tags/list"),
            "/v2/{name}/tags/list"
        );
        assert_eq!(normalize_endpoint("/health"), "/health");
        assert_eq!(normalize_endpoint("/metrics"), "/metrics");
    }
}

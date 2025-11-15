use std::sync::Arc;

use axum::{
    routing::{delete, get, head, patch, post, put},
    Router,
};
use clap::Parser;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod admin;
mod args;
mod auth;
mod blobs;
mod errors;
mod gc;
mod health;
mod manifests;
mod meta;
mod metrics;
mod middleware;
mod openapi;
mod permissions;
mod response;
mod state;
mod storage;
mod tags;
mod utils;
mod validation;

#[tokio::main]
async fn main() {
    let args = args::Args::parse();
    env_logger::init();
    log::info!("Starting grain build: {}", utils::get_build_info());

    // Shared app state
    let shared_state = Arc::new(state::new_app(&args));

    let app = Router::new()
        .route("/", get(meta::index)) // Index, info
        // Health endpoints (no auth required)
        .route("/health", get(health::health))
        .route("/health/live", get(health::liveness))
        .route("/health/ready", get(health::readiness))
        // Metrics endpoint (no auth for Prometheus scraping)
        .route("/metrics", get(metrics::metrics))
        .route("/v2/", get(auth::get)) // end-1
        .route(
            "/v2/{org}/{repo}/manifests/{reference}",
            head(manifests::head_manifest_by_reference),
        )
        .route(
            "/v2/{org}/{repo}/manifests/{reference}",
            get(manifests::get_manifest_by_reference),
        )
        .route(
            "/v2/{org}/{repo}/blobs/{digest}",
            get(blobs::get_blob_by_digest),
        ) // end-2
        .route(
            "/v2/{org}/{repo}/blobs/{digest}",
            head(blobs::head_blob_by_digest),
        )
        .route(
            "/v2/{org}/{repo}/blobs/uploads/",
            post(blobs::post_blob_upload),
        ) // end-4a, end-4b, end-11
        .route(
            "/v2/{org}/{repo}/blobs/uploads/{reference}",
            patch(blobs::patch_blob_upload),
        ) // end-5
        .route(
            "/v2/{org}/{repo}/blobs/uploads/{reference}",
            put(blobs::put_blob_upload_by_reference),
        ) // end-6
        .route(
            "/v2/{org}/{repo}/manifests/{reference}",
            put(manifests::put_manifest_by_reference),
        ) // end-7
        .route("/v2/{org}/{repo}/tags/list", get(tags::get_tags_list)) // end-8a, end-8b
        .route(
            "/v2/{org}/{repo}/manifests/{reference}",
            delete(manifests::delete_manifest_by_reference),
        ) // end-9
        .route(
            "/v2/{org}/{repo}/blobs/{digest}",
            delete(blobs::delete_blob_by_digest),
        ) // end-10
        // Admin API routes
        .route("/admin/users", get(admin::list_users))
        .route("/admin/users", post(admin::create_user))
        .route("/admin/users/{username}", delete(admin::delete_user))
        .route(
            "/admin/users/{username}/permissions",
            post(admin::add_permission),
        )
        .route("/admin/gc", post(admin::run_garbage_collection))
        // Catch-all routes for debugging
        .route("/{*path}", head(meta::catch_all_head))
        .route("/{*path}", get(meta::catch_all_get))
        .route("/{*path}", post(meta::catch_all_post))
        .route("/{*path}", put(meta::catch_all_put))
        .route("/{*path}", patch(meta::catch_all_patch))
        .route("/{*path}", delete(meta::catch_all_delete))
        .with_state(shared_state)
        .layer(axum::middleware::from_fn(middleware::track_metrics))
        .layer(CorsLayer::permissive())
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", openapi::AdminApiDoc::openapi()),
        );

    log::info!("Listening on: {}", &args.host);
    let listener = tokio::net::TcpListener::bind(&args.host).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

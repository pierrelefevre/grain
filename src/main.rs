use std::sync::Arc;

use axum::{
    routing::{delete, get, head, patch, post, put},
    Router,
};
use clap::Parser;
use tower_http::cors::CorsLayer;

mod args;
mod auth;
mod blobs;
mod manifests;
mod meta;
mod response;
mod state;
mod storage;
mod tags;
mod utils;

#[tokio::main]
async fn main() {
    let args = args::Args::parse();
    env_logger::init();
    log::info!("Starting grain build: {}", utils::get_build_info());

    // Shared app state
    let shared_state = Arc::new(state::new_app(&args));

    let app = Router::new()
        .route("/", get(meta::index)) // Index, info
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
        // Catch-all routes for debugging
        .route("/{*path}", head(meta::catch_all_head))
        .route("/{*path}", get(meta::catch_all_get))
        .route("/{*path}", post(meta::catch_all_post))
        .route("/{*path}", put(meta::catch_all_put))
        .route("/{*path}", patch(meta::catch_all_patch))
        .route("/{*path}", delete(meta::catch_all_delete))
        .layer(CorsLayer::permissive())
        .with_state(shared_state);

    log::info!("Listening on: {}", &args.host);
    let listener = tokio::net::TcpListener::bind(&args.host).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

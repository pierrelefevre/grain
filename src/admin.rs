use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::{auth, gc, permissions, response, state};

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub permissions: Vec<state::Permission>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AddPermissionRequest {
    pub repository: String,
    pub tag: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AddPermissionWithUsernameRequest {
    pub username: String,
    pub repository: String,
    pub tag: String,
    pub actions: Vec<String>,
}

/// Check if user is admin (has wildcard delete permission)
fn is_admin(user: &state::User) -> bool {
    permissions::has_permission(user, "*", Some("*"), permissions::Action::Delete)
}

/// List all users (admin only)
#[utoipa::path(
    get,
    path = "/admin/users",
    responses(
        (status = 200, description = "List of all users with their permissions", content_type = "application/json"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - admin permission required")
    ),
    security(
        ("basic_auth" = [])
    )
)]
pub async fn list_users(State(state): State<Arc<state::App>>, headers: HeaderMap) -> Response {
    let host = &state.args.host;

    // Authenticate
    let user = match auth::authenticate_user(&state, &headers).await {
        Ok(u) => u,
        Err(_) => return response::unauthorized(host),
    };

    // Check admin permission
    if !is_admin(&user) {
        return response::forbidden();
    }

    // Get users
    let users = state.users.lock().await;
    let user_list: Vec<_> = users
        .iter()
        .map(|u| {
            serde_json::json!({
                "username": u.username,
                "permissions": u.permissions,
            })
        })
        .collect();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "users": user_list
            })
            .to_string(),
        ))
        .unwrap()
}

/// Create new user (admin only)
#[utoipa::path(
    post,
    path = "/admin/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created successfully", content_type = "application/json"),
        (status = 400, description = "Bad request - invalid JSON"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - admin permission required"),
        (status = 409, description = "Conflict - user already exists"),
        (status = 500, description = "Internal server error - failed to save users")
    ),
    security(
        ("basic_auth" = [])
    )
)]
pub async fn create_user(
    State(state): State<Arc<state::App>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let host = &state.args.host;

    // Authenticate
    let user = match auth::authenticate_user(&state, &headers).await {
        Ok(u) => u,
        Err(_) => return response::unauthorized(host),
    };

    // Check admin permission
    if !is_admin(&user) {
        return response::forbidden();
    }

    // Parse request
    let req: CreateUserRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Invalid request: {}", e)))
                .unwrap();
        }
    };

    // Create new user
    let new_user = state::User {
        username: req.username.clone(),
        password: req.password,
        permissions: req.permissions,
    };

    // Add to users set
    {
        let mut users = state.users.lock().await;

        // Check if user already exists
        if users.iter().any(|u| u.username == new_user.username) {
            return response::conflict("User already exists");
        }

        users.insert(new_user.clone());
    }

    // Persist to file
    if let Err(e) = save_users(&state).await {
        log::error!("Failed to save users: {}", e);
        return response::internal_error();
    }

    log::info!("Created user: {}", new_user.username);

    Response::builder()
        .status(StatusCode::CREATED)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": new_user.username,
                "permissions": new_user.permissions,
            })
            .to_string(),
        ))
        .unwrap()
}

/// Delete user (admin only)
#[utoipa::path(
    delete,
    path = "/admin/users/{username}",
    params(
        ("username" = String, Path, description = "Username of the user to delete")
    ),
    responses(
        (status = 204, description = "User deleted successfully"),
        (status = 400, description = "Bad request - cannot delete yourself"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - admin permission required"),
        (status = 404, description = "Not found - user does not exist"),
        (status = 500, description = "Internal server error - failed to save users")
    ),
    security(
        ("basic_auth" = [])
    )
)]
pub async fn delete_user(
    State(state): State<Arc<state::App>>,
    Path(username): Path<String>,
    headers: HeaderMap,
) -> Response {
    let host = &state.args.host;

    // Authenticate
    let user = match auth::authenticate_user(&state, &headers).await {
        Ok(u) => u,
        Err(_) => return response::unauthorized(host),
    };

    // Check admin permission
    if !is_admin(&user) {
        return response::forbidden();
    }

    // Prevent deleting yourself
    if user.username == username {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Cannot delete yourself"))
            .unwrap();
    }

    // Remove user
    {
        let mut users = state.users.lock().await;
        let before_len = users.len();
        users.retain(|u| u.username != username);

        if users.len() == before_len {
            return response::not_found();
        }
    }

    // Persist to file
    if let Err(e) = save_users(&state).await {
        log::error!("Failed to save users: {}", e);
        return response::internal_error();
    }

    log::info!("Deleted user: {}", username);

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap()
}

/// Add permission to user (admin only)
#[utoipa::path(
    post,
    path = "/admin/users/{username}/permissions",
    params(
        ("username" = String, Path, description = "Username of the user to add permission to")
    ),
    request_body = AddPermissionRequest,
    responses(
        (status = 200, description = "Permission added successfully", content_type = "application/json"),
        (status = 400, description = "Bad request - invalid JSON"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - admin permission required"),
        (status = 404, description = "Not found - user does not exist"),
        (status = 500, description = "Internal server error - failed to save users")
    ),
    security(
        ("basic_auth" = [])
    )
)]
pub async fn add_permission(
    State(state): State<Arc<state::App>>,
    Path(username): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let host = &state.args.host;

    // Authenticate
    let user = match auth::authenticate_user(&state, &headers).await {
        Ok(u) => u,
        Err(_) => return response::unauthorized(host),
    };

    // Check admin permission
    if !is_admin(&user) {
        return response::forbidden();
    }

    // Parse request
    let req: AddPermissionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Invalid request: {}", e)))
                .unwrap();
        }
    };

    let new_permission = state::Permission {
        repository: req.repository,
        tag: req.tag,
        actions: req.actions,
    };

    // Add permission to user
    {
        let mut users = state.users.lock().await;
        let mut user_found = false;

        // Create new set with updated user
        let updated_users: std::collections::HashSet<_> = users
            .iter()
            .map(|u| {
                if u.username == username {
                    user_found = true;
                    let mut updated = u.clone();
                    updated.permissions.push(new_permission.clone());
                    updated
                } else {
                    u.clone()
                }
            })
            .collect();

        if !user_found {
            return response::not_found();
        }

        *users = updated_users;
    }

    // Persist to file
    if let Err(e) = save_users(&state).await {
        log::error!("Failed to save users: {}", e);
        return response::internal_error();
    }

    log::info!(
        "Added permission for user {}: {:?}",
        username,
        new_permission
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&new_permission).unwrap()))
        .unwrap()
}

/// Add permission to user via body (admin only) - alternative endpoint with username in body
#[utoipa::path(
    post,
    path = "/admin/permissions",
    request_body = AddPermissionWithUsernameRequest,
    responses(
        (status = 201, description = "Permission added successfully", content_type = "application/json"),
        (status = 400, description = "Bad request - invalid JSON"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - admin permission required"),
        (status = 404, description = "Not found - user does not exist"),
        (status = 500, description = "Internal server error - failed to save users")
    ),
    security(
        ("basic_auth" = [])
    )
)]
pub async fn add_permission_with_username(
    State(state): State<Arc<state::App>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let host = &state.args.host;

    // Authenticate
    let user = match auth::authenticate_user(&state, &headers).await {
        Ok(u) => u,
        Err(_) => return response::unauthorized(host),
    };

    // Check admin permission
    if !is_admin(&user) {
        return response::forbidden();
    }

    // Parse request
    let req: AddPermissionWithUsernameRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Invalid request: {}", e)))
                .unwrap();
        }
    };

    let new_permission = state::Permission {
        repository: req.repository,
        tag: req.tag,
        actions: req.actions,
    };

    // Add permission to user
    {
        let mut users = state.users.lock().await;
        let mut user_found = false;

        // Create new set with updated user
        let updated_users: std::collections::HashSet<_> = users
            .iter()
            .map(|u| {
                if u.username == req.username {
                    user_found = true;
                    let mut updated = u.clone();
                    updated.permissions.push(new_permission.clone());
                    updated
                } else {
                    u.clone()
                }
            })
            .collect();

        if !user_found {
            return response::not_found();
        }

        *users = updated_users;
    }

    // Persist to file
    if let Err(e) = save_users(&state).await {
        log::error!("Failed to save users: {}", e);
        return response::internal_error();
    }

    log::info!(
        "Added permission for user {}: {:?}",
        req.username,
        new_permission
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&new_permission).unwrap()))
        .unwrap()
}

/// Save users to file
async fn save_users(state: &Arc<state::App>) -> Result<(), Box<dyn std::error::Error>> {
    let users = state.users.lock().await;

    let users_file = state::UsersFile {
        users: users.iter().cloned().collect(),
    };

    let json = serde_json::to_string_pretty(&users_file)?;
    std::fs::write(&state.args.users_file, json)?;

    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GcQuery {
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default = "default_grace_period")]
    pub grace_period_hours: u64,
}

fn default_grace_period() -> u64 {
    24
}

/// Run garbage collection (admin only)
#[utoipa::path(
    post,
    path = "/admin/gc",
    params(
        ("dry_run" = Option<bool>, Query, description = "Run in dry-run mode without deleting blobs"),
        ("grace_period_hours" = Option<u64>, Query, description = "Grace period in hours before deleting unreferenced blobs (default: 24)")
    ),
    responses(
        (status = 200, description = "Garbage collection statistics", content_type = "application/json"),
        (status = 401, description = "Unauthorized - authentication required"),
        (status = 403, description = "Forbidden - admin permission required"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("basic_auth" = [])
    )
)]
pub async fn run_garbage_collection(
    State(state): State<Arc<state::App>>,
    headers: HeaderMap,
    Query(params): Query<GcQuery>,
) -> Response {
    let host = &state.args.host;

    // Authenticate
    let user = match auth::authenticate_user(&state, &headers).await {
        Ok(u) => u,
        Err(_) => return response::unauthorized(host),
    };

    // Check admin permission
    if !is_admin(&user) {
        return response::forbidden();
    }

    let dry_run = params.dry_run;
    let grace_period = params.grace_period_hours;

    log::info!(
        "Admin {} initiated GC (dry_run: {}, grace_period: {}h)",
        user.username,
        dry_run,
        grace_period
    );

    match gc::run_gc(dry_run, grace_period) {
        Ok(stats) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string_pretty(&stats).unwrap()))
            .unwrap(),
        Err(e) => {
            log::error!("GC failed: {}", e);
            response::internal_error()
        }
    }
}

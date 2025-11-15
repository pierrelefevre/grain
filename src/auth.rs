use base64::{prelude::BASE64_STANDARD, Engine};
use std::sync::Arc;

use crate::permissions::{has_permission, Action};
use crate::response::unauthorized;
use crate::state::{self, User};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Response},
};

fn parse_auth_header(headers: &HeaderMap) -> Option<User> {
    let auth_header = headers.get("authorization")?;
    let auth_str = auth_header.to_str().ok()?;
    let auth_decoded_vec = BASE64_STANDARD
        .decode(auth_str.trim_start_matches("Basic "))
        .ok()?;
    let decoded = String::from_utf8(auth_decoded_vec).ok()?;

    let parts: Vec<&str> = decoded.split(':').collect();
    if parts.len() == 2 {
        Some(User {
            username: parts[0].to_string(),
            password: parts[1].to_string(),
            permissions: vec![],
        })
    } else {
        None
    }
}

/// Authenticate user from headers and return User object
pub async fn authenticate_user(state: &Arc<state::App>, headers: &HeaderMap) -> Result<User, ()> {
    let user = parse_auth_header(headers).ok_or(())?;

    let users = state.users.lock().await;
    for u in users.iter() {
        if u.username == user.username && u.password == user.password {
            return Ok(u.clone());
        }
    }

    Err(())
}

/// Check if authenticated user has permission for the action
pub async fn check_permission(
    state: &Arc<state::App>,
    headers: &HeaderMap,
    repository: &str,
    tag: Option<&str>,
    action: Action,
) -> Result<User, ()> {
    // First authenticate
    let user = authenticate_user(state, headers).await?;

    // Then check permission
    if has_permission(&user, repository, tag, action) {
        Ok(user)
    } else {
        log::warn!(
            "User {} denied {} access to {}/{}",
            user.username,
            action.as_str(),
            repository,
            tag.unwrap_or("*")
        );
        Err(())
    }
}

pub(crate) async fn get(State(data): State<Arc<state::App>>, headers: HeaderMap) -> Response<Body> {
    log::info!("Incoming request headers: {:?}", headers);

    match authenticate_user(&data, &headers).await {
        Ok(user) => {
            log::info!("User {} authenticated successfully", user.username);
            Response::builder()
                .status(200)
                .body(Body::from("200 OK"))
                .unwrap()
        }
        Err(_) => {
            log::warn!("Authentication failed");
            unauthorized(&data.args.host)
        }
    }
}

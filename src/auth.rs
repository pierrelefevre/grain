use base64::{prelude::BASE64_STANDARD, Engine};
use std::sync::Arc;

use crate::response::{ok, unauthorized};
use crate::state::{self, User};
use axum::{
    extract::State,
    http::{HeaderMap, Response},
};

fn parse_auth_header(headers: HeaderMap) -> Option<User> {
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
        })
    } else {
        None
    }
}

pub(crate) async fn get(
    State(data): State<Arc<state::App>>,
    headers: HeaderMap,
) -> Response<String> {
    log::info!("Incoming request headers: {:?}", headers);

    let user = match parse_auth_header(headers) {
        Some(user) => {
            log::info!("Parsed user from headers: {:?}", user);
            user
        }
        None => {
            log::warn!("Failed to parse user from headers");
            return unauthorized(&data.args.host);
        }
    };

    let users = data.users.lock().await;

    for u in users.iter() {
        if u.username == user.username && u.password == user.password {
            log::info!("User authenticated successfully");
            return ok();
        }
    }

    unauthorized(&data.args.host)
}

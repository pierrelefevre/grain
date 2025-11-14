use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use std::{collections::HashSet, fmt, fs};

use crate::args::Args;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub(crate) enum ServerStatus {
    Starting,
    Ready,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub(crate) struct User {
    pub(crate) username: String,
    pub(crate) password: String,
}

impl fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerStatus::Starting => write!(f, "Starting"),
            ServerStatus::Ready => write!(f, "Ready"),
        }
    }
}

pub(crate) struct App {
    pub(crate) server_status: Mutex<ServerStatus>,
    pub(crate) users: Mutex<HashSet<User>>,
    pub(crate) args: Args,
}

fn load_users_from_file(file_path: &str) -> HashSet<User> {
    let file_content = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(err) => {
            log::error!("Failed to read users file {}: {}", file_path, err);
            return HashSet::new();
        }
    };

    let users: Vec<User> = match serde_json::from_str(&file_content) {
        Ok(users) => users,
        Err(err) => {
            log::error!(
                "Failed to parse JSON from users file {}: {}",
                file_path,
                err
            );
            return HashSet::new();
        }
    };

    log::info!("Loaded {} users", users.len());
    HashSet::from_iter(users)
}

pub(crate) fn new_app(args: &Args) -> App {
    App {
        server_status: Mutex::new(ServerStatus::Starting),
        users: Mutex::new(load_users_from_file(&args.users_file)),
        args: args.clone(),
    }
}

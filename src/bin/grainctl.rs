use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde_json::json;
use std::process;

#[derive(Parser)]
#[command(name = "grainctl")]
#[command(about = "CLI tool for administering the grain OCI registry", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// User management
    User {
        #[command(subcommand)]
        command: UserCommands,
    },

    /// Run garbage collection
    Gc {
        #[arg(long, default_value = "false")]
        dry_run: bool,

        #[arg(long, default_value = "24")]
        grace_period_hours: u64,

        #[arg(long, env = "GRAIN_URL")]
        url: String,

        #[arg(long, env = "GRAIN_ADMIN_USER")]
        username: String,

        #[arg(long, env = "GRAIN_ADMIN_PASSWORD")]
        password: String,
    },
}

#[derive(Subcommand)]
enum UserCommands {
    /// List all users
    List {
        #[arg(long, env = "GRAIN_URL")]
        url: String,

        #[arg(long, env = "GRAIN_ADMIN_USER")]
        username: String,

        #[arg(long, env = "GRAIN_ADMIN_PASSWORD")]
        password: String,
    },

    /// Create a new user
    Create {
        /// Username for the new user
        user: String,

        /// Password for the new user
        #[arg(long)]
        pass: String,

        #[arg(long, env = "GRAIN_URL")]
        url: String,

        #[arg(long, env = "GRAIN_ADMIN_USER")]
        username: String,

        #[arg(long, env = "GRAIN_ADMIN_PASSWORD")]
        password: String,
    },

    /// Delete a user
    Delete {
        /// Username to delete
        user: String,

        #[arg(long, env = "GRAIN_URL")]
        url: String,

        #[arg(long, env = "GRAIN_ADMIN_USER")]
        username: String,

        #[arg(long, env = "GRAIN_ADMIN_PASSWORD")]
        password: String,
    },

    /// Add permission to a user
    AddPermission {
        /// Target username
        user: String,

        /// Repository pattern (e.g., "myorg/myrepo" or "myorg/*")
        #[arg(long)]
        repository: String,

        /// Tag pattern (e.g., "latest" or "v*")
        #[arg(long)]
        tag: String,

        /// Actions (comma-separated: pull,push,delete)
        #[arg(long)]
        actions: String,

        #[arg(long, env = "GRAIN_URL")]
        url: String,

        #[arg(long, env = "GRAIN_ADMIN_USER")]
        username: String,

        #[arg(long, env = "GRAIN_ADMIN_PASSWORD")]
        password: String,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = execute_command(&cli.command) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn execute_command(cmd: &Commands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::User { command } => execute_user_command(command),
        Commands::Gc {
            dry_run,
            grace_period_hours,
            url,
            username,
            password,
        } => execute_gc_command(*dry_run, *grace_period_hours, url, username, password),
    }
}

fn execute_user_command(cmd: &UserCommands) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    match cmd {
        UserCommands::List {
            url,
            username,
            password,
        } => {
            let response = client
                .get(format!("{}/admin/users", url))
                .basic_auth(username, Some(password))
                .send()?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response
                    .text()
                    .unwrap_or_else(|_| String::from("No response body"));
                return Err(format!("{} - {}", status, text).into());
            }

            let users: serde_json::Value = response.json()?;
            println!("{}", serde_json::to_string_pretty(&users)?);
            Ok(())
        }

        UserCommands::Create {
            user,
            pass,
            url,
            username,
            password,
        } => {
            let body = json!({
                "username": user,
                "password": pass,
                "permissions": []
            });

            let response = client
                .post(format!("{}/admin/users", url))
                .basic_auth(username, Some(password))
                .json(&body)
                .send()?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response
                    .text()
                    .unwrap_or_else(|_| String::from("No response body"));
                return Err(format!("{} - {}", status, text).into());
            }

            println!("User '{}' created successfully", user);
            Ok(())
        }

        UserCommands::Delete {
            user,
            url,
            username,
            password,
        } => {
            let response = client
                .delete(format!("{}/admin/users/{}", url, user))
                .basic_auth(username, Some(password))
                .send()?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response
                    .text()
                    .unwrap_or_else(|_| String::from("No response body"));
                return Err(format!("{} - {}", status, text).into());
            }

            println!("User '{}' deleted successfully", user);
            Ok(())
        }

        UserCommands::AddPermission {
            user,
            repository,
            tag,
            actions,
            url,
            username,
            password,
        } => {
            let actions_vec: Vec<String> =
                actions.split(',').map(|s| s.trim().to_string()).collect();

            let body = json!({
                "repository": repository,
                "tag": tag,
                "actions": actions_vec
            });

            let response = client
                .post(format!("{}/admin/users/{}/permissions", url, user))
                .basic_auth(username, Some(password))
                .json(&body)
                .send()?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response
                    .text()
                    .unwrap_or_else(|_| String::from("No response body"));
                return Err(format!("{} - {}", status, text).into());
            }

            println!(
                "Permission added to user '{}': {} on {}:{}",
                user, actions, repository, tag
            );
            Ok(())
        }
    }
}

fn execute_gc_command(
    dry_run: bool,
    grace_period_hours: u64,
    url: &str,
    username: &str,
    password: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let response = client
        .post(format!(
            "{}/admin/gc?dry_run={}&grace_period_hours={}",
            url, dry_run, grace_period_hours
        ))
        .basic_auth(username, Some(password))
        .send()?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response
            .text()
            .unwrap_or_else(|_| String::from("No response body"));
        return Err(format!("{} - {}", status, text).into());
    }

    let stats: serde_json::Value = response.json()?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    Ok(())
}

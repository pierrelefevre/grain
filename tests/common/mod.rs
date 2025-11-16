use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[allow(dead_code)]
pub struct TestServer {
    pub base_url: String,
    pub host: String,
    pub port: u16,
    pub temp_dir: TempDir,
    pub users_file: PathBuf,
    process: Option<Child>,
}

impl TestServer {
    pub fn new() -> Self {
        Self::new_with_users(default_test_users())
    }

    pub fn new_with_users(users_json: serde_json::Value) -> Self {
        // Find available port
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let host = format!("127.0.0.1:{}", port);
        let base_url = format!("http://{}", host);

        // Create isolated temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Setup storage directories
        std::fs::create_dir_all(temp_path.join("blobs")).unwrap();
        std::fs::create_dir_all(temp_path.join("manifests")).unwrap();
        std::fs::create_dir_all(temp_path.join("uploads")).unwrap();

        // Create users.json
        let users_file = temp_path.join("users.json");
        std::fs::write(
            &users_file,
            serde_json::to_string_pretty(&users_json).unwrap(),
        )
        .expect("Failed to write users.json");

        TestServer {
            base_url,
            host,
            port,
            temp_dir,
            users_file,
            process: None,
        }
    }

    pub fn start(&mut self) {
        // Get the workspace root directory
        let workspace_root = std::env::current_dir().expect("Failed to get current directory");
        
        // Build if not already built
        let build_status = Command::new("cargo")
            .args(["build", "--bin", "grain"])
            .current_dir(&workspace_root)
            .status()
            .expect("Failed to build grain");

        assert!(build_status.success(), "Failed to build grain binary");

        // Path to the binary
        let binary_path = workspace_root.join("target/debug/grain");
        assert!(binary_path.exists(), "grain binary not found at {:?}", binary_path);

        // Change to temp directory for storage
        let temp_path = self.temp_dir.path();

        // Start server process
        let mut child = Command::new(binary_path)
            .args([
                "--host",
                &self.host,
                "--users-file",
                self.users_file.to_str().unwrap(),
            ])
            .current_dir(temp_path)
            .spawn()
            .expect("Failed to start grain server");

        // Wait for server to be ready
        let client = reqwest::blocking::Client::new();
        let url = format!("{}/v2/", self.base_url);

        for _ in 0..50 {
            thread::sleep(Duration::from_millis(100));

            // Check if process is still running
            if let Ok(Some(_)) = child.try_wait() {
                panic!("Server process exited prematurely");
            }

            // Try to connect
            if client
                .get(&url)
                .basic_auth("admin", Some("admin"))
                .send()
                .is_ok()
            {
                self.process = Some(child);
                return;
            }
        }

        // Kill the process if startup failed
        let _ = child.kill();
        panic!("Server failed to start within timeout");
    }

    pub fn stop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }

    pub fn client(&self) -> TestClient {
        TestClient {
            base_url: self.base_url.clone(),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct TestClient {
    pub base_url: String,
    client: reqwest::blocking::Client,
}

#[allow(dead_code)]
impl TestClient {
    pub fn get(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.get(format!("{}{}", self.base_url, path))
    }

    pub fn head(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.head(format!("{}{}", self.base_url, path))
    }

    pub fn post(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.post(format!("{}{}", self.base_url, path))
    }

    pub fn put(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.put(format!("{}{}", self.base_url, path))
    }

    pub fn patch(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.patch(format!("{}{}", self.base_url, path))
    }

    pub fn delete(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.delete(format!("{}{}", self.base_url, path))
    }
}

pub fn default_test_users() -> serde_json::Value {
    serde_json::json!({
        "users": [
            {
                "username": "admin",
                "password": "admin",
                "permissions": [
                    {
                        "repository": "*",
                        "tag": "*",
                        "actions": ["pull", "push", "delete"]
                    }
                ]
            },
            {
                "username": "reader",
                "password": "reader",
                "permissions": [
                    {
                        "repository": "test/*",
                        "tag": "*",
                        "actions": ["pull"]
                    }
                ]
            },
            {
                "username": "writer",
                "password": "writer",
                "permissions": [
                    {
                        "repository": "test/*",
                        "tag": "*",
                        "actions": ["pull", "push"]
                    }
                ]
            },
            {
                "username": "limited",
                "password": "limited",
                "permissions": [
                    {
                        "repository": "myorg/myrepo",
                        "tag": "v*",
                        "actions": ["pull"]
                    }
                ]
            }
        ]
    })
}

pub fn sample_blob() -> Vec<u8> {
    b"This is a test blob content".to_vec()
}

pub fn sample_blob_digest() -> String {
    format!("sha256:{}", sha256::digest("This is a test blob content"))
}

pub fn sample_manifest() -> serde_json::Value {
    let blob_digest = sample_blob_digest();
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "size": 27,
            "digest": blob_digest
        },
        "layers": [
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                "size": 27,
                "digest": blob_digest
            }
        ]
    })
}

pub fn sample_manifest_digest(manifest: &serde_json::Value) -> String {
    let manifest_bytes = serde_json::to_vec(manifest).unwrap();
    format!("sha256:{}", sha256::digest(&manifest_bytes))
}

pub fn sample_image_index() -> serde_json::Value {
    let manifest_digest = sample_manifest_digest(&sample_manifest());
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "size": 500,
                "digest": manifest_digest,
                "platform": {
                    "architecture": "amd64",
                    "os": "linux"
                }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_lifecycle() {
        let mut server = TestServer::new();
        server.start();

        let client = server.client();
        let resp = client
            .get("/v2/")
            .basic_auth("admin", Some("admin"))
            .send()
            .unwrap();

        assert_eq!(resp.status(), 200);

        server.stop();
    }

    #[test]
    fn test_sample_data() {
        let blob = sample_blob();
        assert!(!blob.is_empty());

        let digest = sample_blob_digest();
        assert!(digest.starts_with("sha256:"));

        let manifest = sample_manifest();
        assert_eq!(manifest["schemaVersion"], 2);

        let index = sample_image_index();
        assert_eq!(index["schemaVersion"], 2);
    }
}

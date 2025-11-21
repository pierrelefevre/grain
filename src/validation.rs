use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OciImageManifest {
    pub schema_version: u32,
    pub media_type: Option<String>,
    pub config: Descriptor,
    pub layers: Vec<Descriptor>,
    #[serde(default)]
    pub annotations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OciImageIndex {
    pub schema_version: u32,
    pub media_type: Option<String>,
    pub manifests: Vec<Descriptor>,
    #[serde(default)]
    pub annotations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub media_type: String,
    pub size: u64,
    pub digest: String,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub annotations: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Platform {
    pub architecture: String,
    pub os: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_features: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

#[derive(Debug)]
pub enum ValidationError {
    InvalidJson(String),
    InvalidSchema(String),
    InvalidDigest(String),
    InvalidMediaType(String),
    MissingRequiredField(String),
    InvalidSize(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidJson(msg) => write!(f, "Invalid JSON: {}", msg),
            ValidationError::InvalidSchema(msg) => write!(f, "Invalid schema: {}", msg),
            ValidationError::InvalidDigest(msg) => write!(f, "Invalid digest: {}", msg),
            ValidationError::InvalidMediaType(msg) => write!(f, "Invalid media type: {}", msg),
            ValidationError::MissingRequiredField(msg) => {
                write!(f, "Missing required field: {}", msg)
            }
            ValidationError::InvalidSize(msg) => write!(f, "Invalid size: {}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate manifest JSON and return the detected media type
pub fn validate_manifest(manifest_bytes: &[u8]) -> Result<String, ValidationError> {
    // Parse as generic JSON first
    let manifest_str = std::str::from_utf8(manifest_bytes)
        .map_err(|e| ValidationError::InvalidJson(e.to_string()))?;

    let value: serde_json::Value = serde_json::from_str(manifest_str)
        .map_err(|e| ValidationError::InvalidJson(e.to_string()))?;

    // Check schema version
    let schema_version = value
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ValidationError::MissingRequiredField("schemaVersion".to_string()))?;

    if schema_version != 2 {
        return Err(ValidationError::InvalidSchema(format!(
            "Unsupported schema version: {}",
            schema_version
        )));
    }

    // Detect manifest type by mediaType
    let media_type = value
        .get("mediaType")
        .and_then(|v| v.as_str())
        .unwrap_or(""); // Some manifests omit mediaType

    match media_type {
        "application/vnd.oci.image.manifest.v1+json" => {
            validate_oci_image_manifest(manifest_str)?;
            Ok(media_type.to_string())
        }
        "application/vnd.oci.image.index.v1+json" => {
            validate_oci_image_index(manifest_str)?;
            Ok(media_type.to_string())
        }
        "application/vnd.docker.distribution.manifest.v2+json" => {
            validate_docker_manifest_v2(manifest_str)?;
            Ok(media_type.to_string())
        }
        "application/vnd.docker.distribution.manifest.list.v2+json" => {
            validate_docker_manifest_list(manifest_str)?;
            Ok(media_type.to_string())
        }
        "" => {
            // Try to infer type from content
            if value.get("config").is_some() {
                validate_oci_image_manifest(manifest_str)?;
                Ok("application/vnd.oci.image.manifest.v1+json".to_string())
            } else if value.get("manifests").is_some() {
                validate_oci_image_index(manifest_str)?;
                Ok("application/vnd.oci.image.index.v1+json".to_string())
            } else {
                Err(ValidationError::InvalidSchema(
                    "Cannot determine manifest type".to_string(),
                ))
            }
        }
        _ => Err(ValidationError::InvalidMediaType(format!(
            "Unsupported media type: {}",
            media_type
        ))),
    }
}

fn validate_oci_image_manifest(manifest_str: &str) -> Result<(), ValidationError> {
    let manifest: OciImageManifest = serde_json::from_str(manifest_str)
        .map_err(|e| ValidationError::InvalidSchema(e.to_string()))?;

    // Validate config descriptor
    validate_descriptor(&manifest.config)?;

    // Validate layer descriptors
    if manifest.layers.is_empty() {
        return Err(ValidationError::InvalidSchema(
            "Manifest must have at least one layer".to_string(),
        ));
    }

    for layer in &manifest.layers {
        validate_descriptor(layer)?;
    }

    Ok(())
}

fn validate_oci_image_index(manifest_str: &str) -> Result<(), ValidationError> {
    let index: OciImageIndex = serde_json::from_str(manifest_str)
        .map_err(|e| ValidationError::InvalidSchema(e.to_string()))?;

    // Validate manifest descriptors
    if index.manifests.is_empty() {
        return Err(ValidationError::InvalidSchema(
            "Image index must have at least one manifest".to_string(),
        ));
    }

    for manifest_desc in &index.manifests {
        validate_descriptor(manifest_desc)?;
    }

    Ok(())
}

fn validate_docker_manifest_v2(manifest_str: &str) -> Result<(), ValidationError> {
    // Docker v2 schema is similar to OCI
    validate_oci_image_manifest(manifest_str)
}

fn validate_docker_manifest_list(manifest_str: &str) -> Result<(), ValidationError> {
    // Docker manifest list is similar to OCI image index
    validate_oci_image_index(manifest_str)
}

fn validate_descriptor(desc: &Descriptor) -> Result<(), ValidationError> {
    // Validate digest format (algorithm:hex)
    validate_digest(&desc.digest)?;

    // Validate size is non-zero
    if desc.size == 0 {
        return Err(ValidationError::InvalidSize(
            "Descriptor size must be greater than 0".to_string(),
        ));
    }

    // Validate media type is not empty
    if desc.media_type.is_empty() {
        return Err(ValidationError::InvalidMediaType(
            "Descriptor media type cannot be empty".to_string(),
        ));
    }

    Ok(())
}

fn validate_digest(digest: &str) -> Result<(), ValidationError> {
    lazy_static::lazy_static! {
        // Static regex compilation - safe to unwrap as pattern is hardcoded and valid
        static ref DIGEST_REGEX: Regex = Regex::new(r"^[a-z0-9]+:[a-f0-9]{32,}$").unwrap();
    }

    if !DIGEST_REGEX.is_match(digest) {
        return Err(ValidationError::InvalidDigest(format!(
            "Invalid digest format: {}",
            digest
        )));
    }

    // Check common algorithms
    if !digest.starts_with("sha256:") && !digest.starts_with("sha512:") {
        return Err(ValidationError::InvalidDigest(format!(
            "Unsupported digest algorithm in: {}",
            digest
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_oci_manifest() {
        let manifest = r#"{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "size": 123,
                "digest": "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            },
            "layers": [
                {
                    "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                    "size": 456,
                    "digest": "sha256:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                }
            ]
        }"#;

        assert!(validate_manifest(manifest.as_bytes()).is_ok());
    }

    #[test]
    fn test_invalid_schema_version() {
        let manifest = r#"{"schemaVersion": 1}"#;
        assert!(validate_manifest(manifest.as_bytes()).is_err());
    }

    #[test]
    fn test_invalid_digest() {
        let manifest = r#"{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "size": 123,
                "digest": "invalid-digest"
            },
            "layers": []
        }"#;

        assert!(validate_manifest(manifest.as_bytes()).is_err());
    }

    #[test]
    fn test_empty_layers() {
        let manifest = r#"{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "size": 123,
                "digest": "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            },
            "layers": []
        }"#;

        assert!(validate_manifest(manifest.as_bytes()).is_err());
    }

    #[test]
    fn test_valid_oci_index() {
        let manifest = r#"{
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [
                {
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "size": 123,
                    "digest": "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                }
            ]
        }"#;

        assert!(validate_manifest(manifest.as_bytes()).is_ok());
    }

    #[test]
    fn test_inferred_type() {
        let manifest = r#"{
            "schemaVersion": 2,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "size": 123,
                "digest": "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            },
            "layers": [
                {
                    "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                    "size": 456,
                    "digest": "sha256:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                }
            ]
        }"#;

        let result = validate_manifest(manifest.as_bytes());
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            "application/vnd.oci.image.manifest.v1+json"
        );
    }
}

use axum::{body::Body, http::StatusCode, response::IntoResponse, response::Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    #[serde(rename = "BLOB_UNKNOWN")]
    BlobUnknown,

    #[serde(rename = "BLOB_UPLOAD_INVALID")]
    BlobUploadInvalid,

    #[serde(rename = "BLOB_UPLOAD_UNKNOWN")]
    BlobUploadUnknown,

    #[serde(rename = "DIGEST_INVALID")]
    DigestInvalid,

    #[serde(rename = "MANIFEST_BLOB_UNKNOWN")]
    ManifestBlobUnknown,

    #[serde(rename = "MANIFEST_INVALID")]
    ManifestInvalid,

    #[serde(rename = "MANIFEST_UNKNOWN")]
    ManifestUnknown,

    #[serde(rename = "MANIFEST_UNVERIFIED")]
    ManifestUnverified,

    #[serde(rename = "NAME_INVALID")]
    NameInvalid,

    #[serde(rename = "NAME_UNKNOWN")]
    NameUnknown,

    #[serde(rename = "SIZE_INVALID")]
    SizeInvalid,

    #[serde(rename = "TAG_INVALID")]
    TagInvalid,

    #[serde(rename = "UNAUTHORIZED")]
    Unauthorized,

    #[serde(rename = "DENIED")]
    Denied,

    #[serde(rename = "UNSUPPORTED")]
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OciErrorResponse {
    pub errors: Vec<OciError>,
}

impl OciErrorResponse {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            errors: vec![OciError {
                code,
                message: message.into(),
                detail: None,
            }],
        }
    }

    pub fn with_detail(
        code: ErrorCode,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            errors: vec![OciError {
                code,
                message: message.into(),
                detail: Some(detail.into()),
            }],
        }
    }

    pub fn to_response(&self, status: StatusCode) -> Response {
        let json = serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"errors":[{"code":"UNKNOWN","message":"internal error"}]}"#.to_string()
        });

        Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Body::from(json))
            .unwrap()
    }
}

impl IntoResponse for OciErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.errors.first() {
            Some(err) => match err.code {
                ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
                ErrorCode::Denied => StatusCode::FORBIDDEN,
                ErrorCode::BlobUnknown
                | ErrorCode::ManifestUnknown
                | ErrorCode::NameUnknown
                | ErrorCode::BlobUploadUnknown => StatusCode::NOT_FOUND,
                ErrorCode::DigestInvalid
                | ErrorCode::ManifestInvalid
                | ErrorCode::NameInvalid
                | ErrorCode::TagInvalid
                | ErrorCode::SizeInvalid
                | ErrorCode::BlobUploadInvalid => StatusCode::BAD_REQUEST,
                ErrorCode::Unsupported => StatusCode::METHOD_NOT_ALLOWED,
                ErrorCode::ManifestBlobUnknown | ErrorCode::ManifestUnverified => {
                    StatusCode::BAD_REQUEST
                }
            },
            None => StatusCode::INTERNAL_SERVER_ERROR,
        };

        self.to_response(status)
    }
}

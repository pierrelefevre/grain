// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
// | end-8a | `GET`          | `/v2/<name>/tags/list`                                       | `200`       | `404`             |
// | end-8b | `GET`          | `/v2/<name>/tags/list?n=<integer>&last=<integer>`            | `200`       | `404`             |

use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use std::sync::Arc;

use crate::{auth, permissions, state, storage};
use axum::extract::{Path, Query, State};

// end-8a GET /v2/:name/tags/list
// end-8b GET /v2/:name/tags/list?n=<integer>&last=<integer>
#[derive(Deserialize)]
pub(crate) struct TagsQuery {
    pub n: Option<usize>,
    pub last: Option<String>,
}

fn paginate_tags(tags: Vec<String>, n: Option<usize>, last: Option<String>) -> Vec<String> {
    let mut result = tags;

    // Filter tags after 'last' cursor
    if let Some(last_tag) = last {
        result = result
            .into_iter()
            .skip_while(|tag| tag <= &last_tag)
            .collect();
    }

    // Limit to 'n' results
    if let Some(limit) = n {
        result.truncate(limit);
    }

    result
}

pub(crate) async fn get_tags_list(
    State(state): State<Arc<state::App>>,
    Path((org, repo)): Path<(String, String)>,
    Query(params): Query<TagsQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let host = &state.args.host;
    let repository = format!("{}/{}", org, repo);

    // Check permission (Pull for tag listing)
    match auth::check_permission(
        &state,
        &headers,
        &repository,
        None,
        permissions::Action::Pull,
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            return if auth::authenticate_user(&state, &headers).await.is_ok() {
                Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::from("403 Forbidden: Insufficient permissions"))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(
                        "WWW-Authenticate",
                        format!("Basic realm=\"{}\", charset=\"UTF-8\"", host),
                    )
                    .body(Body::from("401 Unauthorized"))
                    .unwrap()
            };
        }
    }

    // Get all tags from storage
    match storage::list_tags(&org, &repo) {
        Ok(all_tags) => {
            // Apply pagination
            let paginated_tags = paginate_tags(all_tags, params.n, params.last);

            // Build response JSON
            let response_body = serde_json::json!({
                "name": format!("{}/{}", org, repo),
                "tags": paginated_tags
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(response_body.to_string()))
                .unwrap()
        }
        Err(e) => {
            log::error!("Failed to list tags for {}/{}: {}", org, repo, e);

            // Return empty list if directory doesn't exist (valid case)
            let response_body = serde_json::json!({
                "name": format!("{}/{}", org, repo),
                "tags": []
            });

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(response_body.to_string()))
                .unwrap()
        }
    }
}

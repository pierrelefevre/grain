// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
// | end-8a | `GET`          | `/v2/<name>/tags/list`                                       | `200`       | `404`             |
// | end-8b | `GET`          | `/v2/<name>/tags/list?n=<integer>&last=<integer>`            | `200`       | `404`             |

use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::state;
use axum::{
    extract::{Path, Query, State},
    response::Json,
};

// end-8a GET /v2/:name/tags/list
// end-8b GET /v2/:name/tags/list?n=<integer>&last=<integer>
#[derive(Deserialize)]
pub(crate) struct End8bQueryParams {
    n: String,
    last: String,
}
pub(crate) async fn get_tags_list(
    State(data): State<Arc<state::App>>,
    Path(name): Path<String>,
    query: Query<End8bQueryParams>,
) -> Json<Value> {
    let status = data.server_status.lock().await;
    log::info!(
        "tags/get_tags_list: name: {}, n: {}, last: {}",
        name,
        query.n,
        query.last
    );
    return Json(json!({
        "not_implemented": format!("name {} n {:?} last {:?} server_status {}", name, query.n, query.last, status)
    }));
}

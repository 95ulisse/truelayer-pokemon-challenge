use std::convert::Infallible;

use serde_json::json;
use tracing::error;
use warp::{http::StatusCode, Rejection, Reply};

/// Wrapper for an [`anyhow::Error`](anyhow::Error) to make it play nice with warp's rejections.
#[derive(Debug)]
pub struct CustomRejection(anyhow::Error);
impl warp::reject::Reject for CustomRejection {}

impl CustomRejection {
  pub fn new(inner: anyhow::Error) -> Self {
    CustomRejection(inner)
  }
}

/// Warp rejection handler.
/// This function is invoked when an error occurs during the processing of a request,
/// and builds a consistent error response.
pub async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Infallible> {
  let code;
  let message;

  if err.is_not_found() {
    code = StatusCode::NOT_FOUND;
    message = "Not Found";
  } else if err.find::<warp::filters::body::BodyDeserializeError>().is_some() {
    code = StatusCode::BAD_REQUEST;
    message = "Invalid Body";
  } else if let Some(CustomRejection(e)) = err.find::<CustomRejection>() {
    error!(error = %e, "Unhandled error: {:?}", e);
    code = StatusCode::INTERNAL_SERVER_ERROR;
    message = "Internal Server Error";
  } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
    code = StatusCode::METHOD_NOT_ALLOWED;
    message = "Method Not Allowed";
  } else {
    error!(error = ?err, "Unhandled error: {:?}", err);
    code = StatusCode::INTERNAL_SERVER_ERROR;
    message = "Internal Server Error";
  }

  Ok(
    warp::reply::with_status(
      warp::reply::json(&json!({
        "message": message
      })),
      code
    )
  )
}
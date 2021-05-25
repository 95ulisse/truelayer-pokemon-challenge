use std::convert::Infallible;

use serde_json::json;
use tracing::error;
use warp::{http::StatusCode, Filter, Reply, Rejection};

use crate::clients::{PokemonClient, ShakespeareClient};

#[derive(Debug)]
struct CustomRejection(anyhow::Error);
impl warp::reject::Reject for CustomRejection {}

/// Handler for the `GET /pokemon/{name}` route.
async fn handle_get_pokemon(
  pokemon_name: String,
  clients: (PokemonClient, ShakespeareClient)
) -> std::result::Result<impl Reply, Rejection>
{
  // First step: get the description of the pokemon
  let description = clients.0.get_pokemon_description(&pokemon_name).await
    .map_err(CustomRejection)?;

  match description {
    None => {

      // Return a 404 if no pokemon has been found
      Err(warp::reject::not_found())

    },
    Some(description) => {

      // Translate the description and compose the final reply
      let translated = clients.1.translate(&description).await
        .map_err(CustomRejection)?;

      Ok(warp::reply::json(&json!({
        "name": &pokemon_name,
        "description": translated.as_str()
      })))
      
    }
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

/// Builds a [`warp::Filter`](warp::Filter) matching all the routes of this application.
pub fn routes(pokemon_client: PokemonClient, shakespeare_client: ShakespeareClient) -> impl Filter<Extract = impl Reply> + Clone {
  
  let with_clients = || {
    warp::any().map(move || (pokemon_client.clone(), shakespeare_client.clone(),))
  };

  // GET /health
  // Healthcheck endpoint.
  let health = warp::path("health")
    .map(|| "Healthy!");

  // GET /pokemon/{string}
  let get_pokemon = warp::path!("pokemon" / String)
    .and(with_clients())
    .and_then(handle_get_pokemon)
    .recover(handle_rejection);

  health.or(get_pokemon)

}
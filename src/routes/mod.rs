pub mod errors;
pub mod pokemons;

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use lru::LruCache;
use prometheus::{Encoder, TextEncoder};
use serde::Serialize;
use tracing::error;
use warp::{http::StatusCode, Filter, Reply, Rejection};

use crate::clients::{PokemonClient, ShakespeareClient};
use crate::routes::errors::CustomRejection;

/// Shared state for all the requests.
#[derive(Clone)]
pub struct State {
  pub pokemon_client: PokemonClient,
  pub shakespeare_client: ShakespeareClient,
  pub cache: Arc<Mutex<LruCache<String, String>>>
}

fn with_state(state: State) -> impl Filter<Extract = (State,), Error = Infallible> + Clone {
  warp::any().map(move || state.clone())
}

async fn json_or_fail<T: Serialize>(obj: T) -> std::result::Result<impl Reply, Rejection> {
  Ok(warp::reply::json(&obj))
}

async fn handle_metrics() -> std::result::Result<impl Reply, Rejection> {
  let mut buffer = Vec::new();
  let metric_families = prometheus::gather();

  if let Err(e) = TextEncoder::new().encode(&metric_families, &mut buffer) {
    error!(error = %e, "Cannot serialize Prometheus metrics");
    return Err(CustomRejection::new(e.into()).into());
  }

  Ok(buffer)
}

/// Builds a [`warp::Filter`](warp::Filter) matching all the routes of this application.
pub fn routes(pokemon_client: PokemonClient, shakespeare_client: ShakespeareClient, pokemon_cache_size: usize) -> impl Filter<Extract = impl Reply> + Clone {
  
  let state = State {
    pokemon_client,
    shakespeare_client,
    cache: Arc::new(Mutex::new(LruCache::new(pokemon_cache_size)))
  };

  // GET /health
  // Healthcheck endpoint.
  let health = warp::path("health")
    .map(|| StatusCode::OK);

  // GET /metrics
  // Prometheus metrics.
  let metrics = warp::path("metrics")
    .and_then(handle_metrics);

  // GET /pokemon/{string}
  // Returns the Shakespearean translation of the description of a Pokemon.
  let get_pokemon = warp::path!("pokemon" / String)
    .and(with_state(state))
    .and_then(pokemons::handle_get_pokemon)
    .and_then(json_or_fail);

  health.or(metrics).or(get_pokemon)
    .recover(errors::handle_rejection)
    .boxed()

}
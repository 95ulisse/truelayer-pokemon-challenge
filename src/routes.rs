use warp::{Filter, Reply};

/// Builds a [`warp::Filter`](warp::Filter) matching all the routes of this application.
pub fn routes() -> impl Filter<Extract = impl Reply> + Clone {
  
  // GET /health
  // Healthcheck endpoint.
  let health = warp::path("health")
    .map(|| "Healthy!");

  // GET /hello/{string}
  let hello = warp::path!("hello" / String)
    .map(|name| format!("Hello, {}!", name));

  health.or(hello)

}
mod routes;
mod clients;
mod metrics;

use std::env;

use anyhow::Result;
use futures::stream::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use tracing::{info, warn, error};
use warp::Filter;

use crate::clients::{PokemonClient, ShakespeareClient};

async fn run() -> Result<()> {
  
  // Register the termination signals handlers
  let mut signals = Signals::new(&[
    SIGTERM,
    SIGINT,
    SIGQUIT,
  ])?;

  // Get the port to bind to from the env
  let port = env::var("PORT")
    .map_err(|_| ())
    .and_then(|s| s.parse::<u16>().map_err(|_| ()))
    .unwrap_or_else(|_| {
      warn!("Invalid or missing PORT env value. Defauling to 8080.");
      8080
    });

  // Extract all the required envs for the clients
  let pokemon_url = env::var("POKEAPI_ENDPOINT")?;
  let pokemon_cache_size = env::var("POKEAPI_CACHE_SIZE")?.parse::<usize>()?;
  let shakespeare_url = env::var("SHAKESPEARE_TRANSLATOR_ENDPOINT")?;

  // Build the clients
  let pokemon_client = PokemonClient::new(&pokemon_url)?;
  let shakespeare_client = ShakespeareClient::new(&shakespeare_url)?;

  // Build the application routes.
  // Also, enable tracing for all requests.
  let r = routes::routes(pokemon_client, shakespeare_client, pokemon_cache_size)
    .with(warp::trace::request());

  // Start the HTTP server and stop it when a termination signal is received
  let (bound_address, server_future) = warp::serve(r)
    .try_bind_with_graceful_shutdown(
      ([ 0, 0, 0, 0 ], port),
      async move {
        signals.next().await;
        info!("Received termination signal. Begin graceful shutdown.");
      }
    )?;
  info!("Server bound on {}", bound_address);
  server_future.await;

  Ok(())

}

#[tokio::main]
async fn main() {

  // Configure tracing collector as soon as possible
  tracing_subscriber::fmt()
    .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned()))
    .init();

  // Delegate to the `run` function
  let exit_code = match run().await {
    Err(e) => {
      error!(error = %e, "Fatal error");
      1
    },
    Ok(()) => {
      info!("Application successfully terminated. Bye!");
      0
    }
  };
  std::process::exit(exit_code);

}
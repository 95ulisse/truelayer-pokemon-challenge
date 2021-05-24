use std::{env, error::Error};
use futures::stream::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use tracing::{info, warn, error};
use warp::Filter;

mod routes;

async fn run()  -> Result<(), Box<dyn Error + 'static>> {
    
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

    // Build the application routes.
    // Also, enable tracing for all requests.
    let r = routes::routes()
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
    tracing_subscriber::fmt().init();

    // Delegate to the `run` function
    let mut exit_code = 0;
    match run().await {
        Err(e) => {
            error!(error = e.as_ref(), "Fatal error");
            exit_code = 1;
        },
        Ok(()) => {
            info!("Application terminated. Bye!");
        }
    }
    std::process::exit(exit_code);

}
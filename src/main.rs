#![deny(warnings)]

mod ai;
mod config;
mod database;
mod models;
mod server;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};

use config::Config;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("autoblogger=info".parse()?))
        .init();

    // Load and validate configuration
    let config = Config::from_env()
        .map_err(|e| format!("Configuration error: {}", e))?;

    // Initialize database with config
    database::init_pool_with_config(&config)
        .map_err(|e| format!("Database initialization error: {}", e))?;

    // This address is localhost
    let addr: SocketAddr = ([0, 0, 0, 0], config.server_port).into();

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr).await?;
    info!("Server listening on http://{}", addr);
    loop {
        // When an incoming TCP connection is received grab a TCP stream for
        // client<->server communication.
        //
        // Note, this is a .await point, this loop will loop forever but is not a busy loop. The
        // .await point allows the Tokio runtime to pull the task off of the thread until the task
        // has work to do. In this case, a connection arrives on the port we are listening on and
        // the task is woken up, at which point the task is then put back on a thread, and is
        // driven forward by the runtime, eventually yielding a TCP stream.
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);

        // Spin up a new task in Tokio so we can continue to listen for new TCP connection on the
        // current task without waiting for the processing of the HTTP1 connection we just received
        // to finish
        let config_clone = config.clone();
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `server::handle_request` function
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| {
                    server::handle_request(req, config_clone.clone())
                }))
                .await
            {
                error!("Error serving connection: {:?}", err);
            }
        });
    }
}
#![deny(warnings)]

mod ai;
mod database;
mod models;
mod server;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;


#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // This address is localhost
    let addr: SocketAddr = ([0, 0, 0, 0], 3000).into();

    env::var("AI_MODEL").expect("AI_MODEL should be set");

    let ai_model = env::var("AI_MODEL").unwrap();

    match ai_model.as_str() {
        "gpt4" => {
            env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY should be set");
        }

        "claude3" => {
            env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY should be set");
        }

        "claude4" => {
            env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY should be set");
        }

        _ => {
            panic!("AI_MODEL should be 'gpt4', 'claude3' or 'claude4'");
        }
    }

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
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
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `server::handle_request` function
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(server::handle_request))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}


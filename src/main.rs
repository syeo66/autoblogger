#![deny(warnings)]

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Choice {
    index: i64,
    message: Message,
}

#[derive(Serialize, Deserialize, Debug)]
struct Completion {
    id: String,
    object: String,
    created: i64,
    model: String,
    choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RequestBody {
    model: String,
    messages: Vec<Message>,
    max_tokens: i64,
}

// An async function that consumes a request, does nothing with it and returns a
// response.
async fn content(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let title = unslugify(req.uri().path());
    let title = capitalize_words(&title);

    // TODO fetch content from database
    // TODO create links for each sentence

    // TODO fetch content from ChatGPT if not found in database
    let content = fetch_content_from_gpt(&title)
        .await
        .unwrap_or("".to_string())
        .replace("\n", "<br>");

    // TODO store content in database
    // TODO fetch random content from database if no path is given

    let body = format!(
        r#"
        <p>{}</p>
        "#,
        content
    );

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="utf-8">
            <title>{}</title>
        </head>
        <body>
            <h1>{}</h1>
            {}
        </body>
        </html>
        "#,
        title,
        title,
        body.trim().replace("\n", "<br>")
    );

    Ok(Response::new(Full::new(Bytes::from(html))))
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // This address is localhost
    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

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

        // Spin up a new task in Tokio so we can continue to listen for new TCP connection on the
        // current task without waiting for the processing of the HTTP1 connection we just received
        // to finish
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `content` function
            if let Err(err) = http1::Builder::new()
                .serve_connection(stream, service_fn(content))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn fetch_content_from_gpt(title: &str) -> Result<String, Box<dyn std::error::Error>> {
    let openai_api_key = env::var("OPENAI_API_KEY").unwrap();
    let model = "gpt-3.5-turbo";
    let api_key = &openai_api_key;
    let url = "https://api.openai.com/v1/chat/completions";
    let prompt = format!("Write a blog post about the following topic: {}", title);

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are a blog author.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        },
    ];

    let headers = build_headers(api_key)?;
    let body: RequestBody = RequestBody {
        model: model.to_string(),
        messages: messages.clone(),
        max_tokens: 2000,
    };

    let client = reqwest::Client::new();
    let response: Completion = client
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    Ok(response.choices[0].message.content.clone())
}

fn build_headers(api_key: &str) -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", api_key))?,
    );
    Ok(headers)
}

fn unslugify(s: &str) -> String {
    s.replace("-", " ")
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

fn capitalize_words(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c.is_whitespace() {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
}

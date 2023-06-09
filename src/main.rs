#![deny(warnings)]

use bytes::Bytes;
use comrak::{
    format_html_with_plugins, parse_document, plugins, Arena, ComrakOptions, ComrakPlugins,
    ComrakRenderOptions,
};
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use rusqlite::{params, Connection};
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
    let conn = Connection::open("./blog.db").expect("Could not open database");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS articles (
            slug     TEXT PRIMARY KEY,
            title    TEXT NOT NULL,
            content  TEXT NOT NULL,
            createdAt DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (), // empty list of parameters.
    )
    .expect("Could not create table");

    let slug = req.uri().path().trim_start_matches('/');
    let title = unslugify(slug);
    let title = capitalize_words(&title);

    // TODO show a list of topics from database if no url is given

    // fetch content from database based on slug
    let result = conn
        .prepare("SELECT content FROM articles WHERE title = ?1 LIMIT 1")
        .expect("Could not prepare query")
        .query_row(params![title], |row| row.get(0));

    // fetch content from ChatGPT if not found in database
    let content = match result {
        Ok(content) => content,
        _ => fetch_content_from_gpt(&title)
            .await
            .unwrap_or("".to_string()),
    };

    let _ = conn.execute(
        "INSERT INTO articles (slug, title, content) VALUES (?1, ?2, ?3)",
        params![slug, title, content],
    );

    let content = markdown_parse(&content);

    // TODO create links for each sentence
    // TODO improve styles

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="utf-8">
            <title>{}</title>
            <style>
                pre {{
                    padding: 0.5rem;
                }}

                body {{
                    margin: 0;
                    padding: 0;
                    width: 100%;
                }}

                article {{
                    padding: 2.5rem;
                }}

                @media screen and (min-width: 768px) {{
                    article {{
                        max-width: 960px;
                        margin: 0 auto;
                    }}
                }}

                p {{
                    margin: 0 0 2rem 0;
                    padding: 0;
                    font-family: sans-serif;
                    line-height: 1.5;
                    hyphens: auto;
                    text-align: justify;
                }}
            </style>
        </head>
        <body>
            <article>
                <h1>{}</h1>
                {}
            </article>
        </body>
        </html>
        "#,
        title,
        title,
        content.trim()
    );

    Ok(Response::new(Full::new(Bytes::from(html))))
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // This address is localhost
    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY should be set");

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
            content: "Create an example blog post to show how links should be used in a blog post about 'More Thoughts On AI'. Format the blog posts using markdown. Add inline links of important parts by using slugs as a relative URL without protocol, host or domain part.".to_string(),
        },
        Message {
            role: "assistant".to_string(),
            content: "Artificial Intelligence (AI) has been a hot topic in recent years, as advances in technology have allowed for greater and more widespread implementation of these systems. While [AI offers many benefits to society](ai-offers-many-benefits-to-society), including increased efficiency and accuracy in various fields ranging from healthcare to finance, there are also concerns about [its potential negative consequences](potential-negative-consequences-of-ai).

One of the major concerns about AI is its potential to displace human workers in certain industries. As AI becomes more advanced, it is likely that it will be able to perform many tasks that are currently done by human workers more efficiently and accurately. While this could lead to lower costs and increased productivity for businesses, it may also lead to job loss and economic disruption for those who are displaced.

".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!("Write a blog entry about '{}'. Format the blog posts using markdown. Add at least 5 inline links of important parts in thext (not at the end) by using slugs as a relative URL without protocol, host or domain part (no https://example.com). Do not repeat the title in the article.", prompt.to_string()),
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

fn markdown_parse(s: &str) -> String {
    let arena = Arena::new();

    let comrak_options = ComrakOptions {
        render: ComrakRenderOptions {
            unsafe_: false,
            escape: false,
            ..ComrakRenderOptions::default()
        },
        ..ComrakOptions::default()
    };

    let root = parse_document(&arena, s, &comrak_options);

    let mut html = vec![];
    let mut plugins = ComrakPlugins::default();
    let adapter = plugins::syntect::SyntectAdapter::new("base16-ocean.dark");

    plugins.render.codefence_syntax_highlighter = Some(&adapter);

    format_html_with_plugins(root, &comrak_options, &mut html, &plugins).unwrap();

    String::from_utf8(html).unwrap()
}

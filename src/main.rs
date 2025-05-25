#![deny(warnings)]

use bytes::Bytes;
use chrono::{DateTime, Duration, Local, NaiveDateTime};
use comrak::{
    format_html_with_plugins, parse_document, plugins, Arena, ComrakOptions, ComrakPlugins,
    ComrakRenderOptions,
};
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
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
struct GptCompletion {
    id: String,
    object: String,
    created: i64,
    model: String,
    choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize, Debug)]
struct AnthropicContent {
    text: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct AnthropicCompletion {
    content: Vec<AnthropicContent>,
    model: String,
    role: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct RequestBody {
    model: String,
    messages: Vec<Message>,
    max_tokens: i64,
}

#[derive(Debug)]
struct Content {
    title: String,
    content: String,
}

// An async function that consumes a request, does nothing with it and returns a
// response.
async fn content(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let db_path = env::var("DB_PATH").unwrap_or("./blog.db".to_string());
    let conn = Connection::open(db_path).expect("Could not open database");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS articles (
            slug     TEXT PRIMARY KEY,
            title    TEXT NOT NULL,
            content  TEXT NOT NULL,
            createdAt DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (), // empty list of parameters.
    )
    .expect("Could not create articles table");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS locks (
            title     TEXT NOT NULL,
            createdAt DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (), // empty list of parameters.
    )
    .expect("Could not create locks table");

    let uri = req.uri().path();
    let route = uri.trim_start_matches('/').trim();

    if route == "robots.txt" {
        return Ok(Response::new(Full::new(Bytes::from(
            "User-agent: *\nDisallow: /",
        ))));
    }

    if route == "favicon.ico" {
        return Ok(Response::new(Full::new(Bytes::from(""))));
    }

    let slug = route;
    let slug = slug
        .replace(".", "-")
        .replace("_", "-")
        .replace("/", "-")
        .to_lowercase();

    println!("==========================================");
    println!("Slug: {}", slug);

    // if slug is empty, return list of articles
    if slug.is_empty() {
        let mut html = String::new();
        html.push_str("<ul class='article-list'>");
        for row in conn
            .prepare("SELECT title, slug FROM articles ORDER BY createdAt DESC LIMIT 20")
            .expect("Could not prepare query")
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("Could not query")
        {
            let (title, slug) = row.unwrap();
            html.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                slug,
                title.trim_matches('"')
            ));
        }
        html.push_str("</ul>");

        let html = apply_layout("Blog", &html);

        return Ok(Response::new(Full::new(Bytes::from(html))));
    }

    // fetch content from database based on slug
    let result = conn
        .prepare("SELECT title, content FROM articles WHERE slug = ?1 LIMIT 1")
        .expect("Could not prepare query")
        .query_row(params![slug], |row| {
            Ok(Content {
                title: row.get(0)?,
                content: row.get(1)?,
            })
        });

    // prevent creating a new article if one was generated in the last 24 hours
    let last = conn
        .prepare(
            "SELECT createdAt FROM articles WHERE createdAt > datetime('now','-1 day') LIMIT 1",
        )
        .expect("Could not prepare query")
        .query_row([], |row| row.get::<usize, String>(0))
        .unwrap_or("".to_string());

    if !last.is_empty() && result.is_err() {
        let date = NaiveDateTime::parse_from_str(&last, "%Y-%m-%d %H:%M:%S").unwrap();
        let current_time = Local::now();
        let offset = current_time.offset().clone();
        let datetime =
            DateTime::<Local>::from_naive_utc_and_offset(date, offset) + Duration::days(1);

        let difference = datetime.signed_duration_since(current_time);
        let hours = difference.num_hours();
        let msg = format!("Only one article can be generated per day. Please wait {} hours before generating a new article.", hours+1);

        let html = apply_layout("Try later", &msg);
        return Ok(Response::new(Full::new(Bytes::from(html))));
    }

    let lock = conn
        .prepare(
            "SELECT createdAt FROM locks WHERE createdAt > datetime('now','-5 minutes') LIMIT 1",
        )
        .expect("Could not prepare query")
        .query_row([], |row| row.get::<usize, String>(0))
        .unwrap_or("".to_string());

    if !lock.is_empty() && result.is_err() {
        let msg = format!("Content creation temporary locked");
        let html = apply_layout("Try later", &msg);
        return Ok(Response::new(Full::new(Bytes::from(html))));
    }

    // fetch content from API if not found in database
    let content = match result {
        Ok(content) => content,
        _ => {
            let _ = conn.execute("INSERT INTO locks (title) VALUES (?1)", params!["lock"]);

            let title = fetch_title(&slug).await.unwrap_or({
                let t = unslugify(&slug);
                capitalize_words(&t)
            });
            let title = title.trim_matches('"');

            fetch_content(&title).await.unwrap_or(Content {
                title: "".to_string(),
                content: "".to_string(),
            })
        }
    };

    if content.content.is_empty() {
        return Ok(Response::new(Full::new(Bytes::from(
            "No content found for this article",
        ))));
    }

    let _ = conn.execute(
        "INSERT INTO articles (slug, title, content) VALUES (?1, ?2, ?3)",
        params![slug, content.title, content.content],
    );

    let raw = if content.content.trim().starts_with("#") {
        remove_first_line(&content.content)
    } else {
        content.content
    };
    let html = markdown_parse(&raw);
    let html = apply_layout(&content.title.trim_matches('"'), &html);

    Ok(Response::new(Full::new(Bytes::from(html))))
}

fn remove_first_line(s: &str) -> String {
    s.lines().clone().skip(1).collect::<Vec<&str>>().join("\n")
}

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
            // HTTP requests received on that connection to the `content` function
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(content))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn fetch_title(slug: &str) -> Result<String, Box<dyn std::error::Error>> {
    match env::var("AI_MODEL").unwrap().as_str() {
        "gpt4" => fetch_title_from_gpt(slug).await,
        "claude3" => fetch_title_from_claude(slug).await,
        "claude4" => fetch_title_from_claude(slug).await,
        _ => Err("AI_MODEL should be 'gpt4', 'claude3' or 'claude4'".into()),
    }
}

async fn fetch_title_from_claude(slug: &str) -> Result<String, Box<dyn std::error::Error>> {
    println!("Fetching title from Claude for slug: {}", slug);

    fetch_from_claude(get_title_messages(slug)).await
}

async fn fetch_from_claude(messages: Vec<Message>) -> Result<String, Box<dyn std::error::Error>> {
    let anthropy_api_key = env::var("ANTHROPIC_API_KEY").unwrap();

    let api_key = &anthropy_api_key;
    let url = "https://api.anthropic.com/v1/messages";

    let model = if env::var("AI_MODEL").unwrap().as_str() == "claude3" {
        "claude-3-7-sonnet-latest"
    } else {
        "claude-sonnet-4-20250514"
    };

    let headers = build_anthropic_headers(api_key)?;
    let body: RequestBody = RequestBody {
        model: model.to_string(),
        messages: messages.clone(),
        max_tokens: 2000,
    };

    let client = reqwest::Client::new();
    let response = client.post(url).headers(headers).json(&body).send().await;
    let response = match response {
        Err(err) => Err(err),
        Ok(response) => response.json::<AnthropicCompletion>().await,
    };

    match response {
        Err(_) => {
            println!("Error: {:?}", response);
            return Err("Error fetching title from Claude".into());
        }
        Ok(response) => Ok(response.content[0].text.clone()),
    }
}

async fn fetch_title_from_gpt(slug: &str) -> Result<String, Box<dyn std::error::Error>> {
    println!("Fetching title from GPT for slug: {}", slug);
    fetch_from_gpt(get_title_messages(slug)).await
}

async fn fetch_from_gpt(messages: Vec<Message>) -> Result<String, Box<dyn std::error::Error>> {
    let openai_api_key = env::var("OPENAI_API_KEY").unwrap();

    let model = "gpt-4o";
    let api_key = &openai_api_key;
    let url = "https://api.openai.com/v1/chat/completions";

    let headers = build_gpt_headers(api_key)?;
    let body: RequestBody = RequestBody {
        model: model.to_string(),
        messages: messages.clone(),
        max_tokens: 2000,
    };

    let client = reqwest::Client::new();
    let response = client.post(url).headers(headers).json(&body).send().await;
    let response = match response {
        Err(err) => Err(err),
        Ok(response) => response.json::<GptCompletion>().await,
    };

    match response {
        Err(response) => {
            println!("Error: {:?}", response);
            return Err("Error fetching title from OpenAI".into());
        }

        Ok(response) => Ok(response.choices[0].message.content.clone()),
    }
}

async fn fetch_content(title: &str) -> Result<Content, Box<dyn std::error::Error>> {
    match env::var("AI_MODEL").unwrap().as_str() {
        "gpt4" => fetch_content_from_gpt(title).await,
        "claude3" => fetch_content_from_claude(title).await,
        "claude4" => fetch_content_from_claude(title).await,
        _ => Err("AI_MODEL should be 'gpt4', 'claude3' or 'claude4'".into()),
    }
}

async fn fetch_content_from_claude(title: &str) -> Result<Content, Box<dyn std::error::Error>> {
    println!("Fetching content from Claude for title: {}", title);

    let messages = get_messages(title);
    let response = fetch_from_claude(messages).await;

    match response {
        Err(_) => {
            println!("Error: {:?}", response);
            return Ok(Content {
                title: "".to_string(),
                content: "".to_string(),
            });
        }
        Ok(response) => Ok(Content {
            title: title.to_string(),
            content: response,
        }),
    }
}

fn build_anthropic_headers(api_key: &str) -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(&format!("{}", api_key))?);
    headers.insert("anthropic-version", HeaderValue::from_str("2023-06-01")?);
    headers.insert("content-type", HeaderValue::from_str("application/json")?);
    Ok(headers)
}

async fn fetch_content_from_gpt(title: &str) -> Result<Content, Box<dyn std::error::Error>> {
    println!("Fetching content from GPT for title: {}", title);

    let messages = get_messages(title);
    let response = fetch_from_gpt(messages).await;

    match response {
        Err(_) => {
            println!("Error: {:?}", response);
            return Ok(Content {
                title: "".to_string(),
                content: "".to_string(),
            });
        }

        Ok(response) => Ok(Content {
            title: title.to_string(),
            content: response,
        }),
    }
}

fn build_gpt_headers(api_key: &str) -> Result<HeaderMap, Box<dyn std::error::Error>> {
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

fn apply_layout(title: &str, content: &str) -> String {
    // TODO add back link
    format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="utf-8">
            <meta name="viewport" content="initial-scale=1, width=device-width">
            <meta name="robots" content="noindex,nofollow">
            <title>{}</title>
            <style>
                pre {{
                    padding: 0.5rem;
                    margin: 0 0 2rem 0;
                }}

                :root {{
                    color-scheme: light dark;  
                }}

                body {{
                    display: flex;
                    flex-direction: column;
                    min-height: 100dvh;
                    margin: 0;
                    padding: 0;
                    width: 100%;
                }}

                article {{
                    padding: 1rem;
                }}

                article :first-child {{
                    margin-top: 0;
                }}

                .article-list {{
                    list-style: none;
                    padding: 0;
                    margin: 0;
                }}

                .article-list li {{
                    border-radius: 0.5rem;
                    border: 1px solid #ccc;
                    font-family: system-ui, ui-sans-serif,  sans-serif;
                    margin: 0 0 1rem 0;
                    padding: 1rem;
                }}

                .article-list li a {{
                    text-decoration: none;
                    font-size: 1.2rem;
                }}

                @media (prefers-color-scheme: light) {{
                    header a {{
                        color: black;
                    }}
                }}
                @media (prefers-color-scheme: dark) {{
                    header a {{
                        color: white;
                    }}
                }}

                header a {{
                    text-decoration: none;
                }}

                p, ol, ul {{
                    font-family: system-ui, ui-sans-serif,  sans-serif;
                    hyphens: auto;
                    line-height: 1.5;
                    margin: 0 0 2rem 0;
                    padding: 0;
                    text-align: justify;
                }}

                ol, ul {{
                    padding: 0 0 0 3rem;
                }}

                header, footer {{
                    padding: 1rem;
                    font-family: system-ui, ui-sans-serif, sans-serif;
                    font-size: 1rem;
                }}
                @media (prefers-color-scheme: light) {{
                    header, footer {{
                        background-color: #f5f5f5;
                    }}
                }}
                @media (prefers-color-scheme: dark) {{
                    header, footer {{
                        background-color: #333;
                    }}
                }}

                footer {{
                    margin-top: auto;
                }}

                header h1 {{
                    margin: 0;
                    padding: 0;
                }}

                @media screen and (min-width: 768px) {{
                    header {{
                        padding: 2.5rem;
                    }}

                    article {{
                        max-width: 960px;
                        margin: 0 auto;
                        padding: 2.5rem;
                        width: 100%;
                    }}
                }}
            </style>
        </head>
        <body>
            <header>
                <h1><a href="/">Autoblogger</a></h1>
            </header>
            <article>
                <h1>{}</h1>
                {}
            </article>
            <footer>
                All articles are automatically generated by a LLM (Large Language Model like GPT4 or Claude3). It can contain hallucinations, nonsense, factual errors, and other inaccuracies. Do not take it seriously in any way. <a href="https://github.com/syeo66/autoblogger">Source on Github</a>
            </footer>
        </body>
        </html>
        "#,
        title,
        title,
        content.trim()
    )
    .trim()
    .into()
}

fn get_title_messages(slug: &str) -> Vec<Message> {
    let prompt = get_title_prompt(slug);
    vec![Message {
        role: "user".to_string(),
        content: prompt.to_string(),
    }]
}

fn get_messages(title: &str) -> Vec<Message> {
    let prompt = get_prompt(title);
    vec![
        Message {
            role: "user".to_string(),
            content: "You are a blog author. Create an example blog post to show how links should be used in a blog post about 'More Thoughts On AI'. Format the blog posts using markdown. Add inline links of important parts by using slugs as a relative URL without protocol, host or domain part.".to_string(),
        },
        Message {
            role: "assistant".to_string(),
            content: "Artificial Intelligence (AI) has been a hot topic in recent years, as advances in technology have allowed for greater and more widespread implementation of these systems. While [AI offers many benefits to society](ai-offers-many-benefits-to-society), including increased efficiency and accuracy in various fields ranging from healthcare to finance, there are also concerns about [its potential negative consequences](potential-negative-consequences-of-ai).

One of the major concerns about AI is its potential to displace human workers in certain industries. As AI becomes more advanced, it is likely that it will be able to perform many tasks that are currently done by human workers more efficiently and accurately. While this could lead to lower costs and increased productivity for businesses, it may also lead to job loss and economic disruption for those who are displaced.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        },
    ]
}

fn get_title_prompt(slug: &str) -> String {
    format!("Write a blog articles title from the slug '{}'. Return only one title. If it contains anything else then one single title it is useless.", slug)
}

fn get_prompt(title: &str) -> String {
    format!("Write a blog entry about the topic '{}'. Format the blog posts using markdown. Add at least 5 inline links of important parts in thext (not at the end) by using slugs as a relative URL without protocol, host or domain part (no https://example.com). Do not repeat the title in the article. If you use the title in the article it is useless.", title)
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

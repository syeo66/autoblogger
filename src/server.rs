use bytes::Bytes;
use comrak::{
    format_html_with_plugins, parse_document, plugins, Arena, ComrakOptions, ComrakPlugins,
    ComrakRenderOptions,
};
use http_body_util::Full;
use hyper::{Request, Response};
use std::convert::Infallible;

use crate::ai::{capitalize_words, fetch_content, fetch_title, unslugify};
use crate::database::{
    calculate_wait_time, check_daily_rate_limit, check_generation_lock, create_generation_lock,
    get_article_by_slug, get_pool, get_recent_articles, insert_article,
};

pub async fn handle_request(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
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

    let slug = route
        .replace(".", "-")
        .replace("_", "-")
        .replace("/", "-")
        .to_lowercase();

    println!("==========================================");
    println!("Slug: {}", slug);

    if slug.is_empty() {
        return handle_article_list().await;
    }

    handle_article_request(&slug).await
}

async fn handle_article_list() -> Result<Response<Full<Bytes>>, Infallible> {
    let pool = get_pool();

    let articles = match get_recent_articles(pool) {
        Ok(articles) => articles,
        Err(_) => {
            return Ok(Response::new(Full::new(Bytes::from(
                "Failed to fetch articles"
            ))));
        }
    };

    let mut html = String::new();
    html.push_str("<ul class='article-list'>");
    
    for (title, slug) in articles {
        html.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>",
            slug,
            title.trim_matches('"')
        ));
    }
    
    html.push_str("</ul>");
    let html = apply_layout("Blog", &html);

    Ok(Response::new(Full::new(Bytes::from(html))))
}

async fn handle_article_request(slug: &str) -> Result<Response<Full<Bytes>>, Infallible> {
    let pool = get_pool();

    let existing_article = get_article_by_slug(pool, slug);

    if let Ok(content) = existing_article {
        let raw = if content.content.trim().starts_with("#") {
            remove_first_line(&content.content)
        } else {
            content.content
        };
        let html = markdown_parse(&raw);
        let html = apply_layout(&content.title.trim_matches('"'), &html);
        return Ok(Response::new(Full::new(Bytes::from(html))));
    }

    if let Ok(Some(last_date)) = check_daily_rate_limit(pool) {
        if let Ok(hours_to_wait) = calculate_wait_time(&last_date) {
            let msg = format!(
                "Only one article can be generated per day. Please wait {} hours before generating a new article.",
                hours_to_wait
            );
            let html = apply_layout("Try later", &msg);
            return Ok(Response::new(Full::new(Bytes::from(html))));
        }
    }

    if let Ok(true) = check_generation_lock(pool) {
        let msg = "Content creation temporary locked".to_string();
        let html = apply_layout("Try later", &msg);
        return Ok(Response::new(Full::new(Bytes::from(html))));
    }

    let _ = create_generation_lock(pool);

    let title = fetch_title(slug).await.unwrap_or_else(|_| {
        let t = unslugify(slug);
        capitalize_words(&t)
    });
    let title = title.trim_matches('"');

    let content = fetch_content(title).await.unwrap_or_else(|_| crate::models::Content {
        title: "".to_string(),
        content: "".to_string(),
    });

    if content.content.is_empty() {
        return Ok(Response::new(Full::new(Bytes::from(
            "No content found for this article",
        ))));
    }

    let _ = insert_article(pool, slug, &content.title, &content.content);

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

fn apply_layout(title: &str, content: &str) -> String {
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
                    margin: 0 0 2rem 0;
                    overflow: auto;
                    padding: 0.5rem;
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
                All articles are automatically generated by a LLM (Large Language Model like GPT-4 or Claude). It can contain hallucinations, nonsense, factual errors, and other inaccuracies. Do not take it seriously in any way. Contact: Red Ochsenbein &lt;autoblogger@control.raven.ch&gt; <a href="https://github.com/syeo66/autoblogger">Source on Github</a>
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
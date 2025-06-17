use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

use crate::config::{AiModel, Config};
use crate::models::{AnthropicCompletion, Content, GptCompletion, Message, RequestBody};

pub async fn fetch_title(slug: &str, config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    match config.ai_model {
        AiModel::Gpt4 => fetch_title_from_gpt(slug, config).await,
        AiModel::Claude3 | AiModel::Claude4 => fetch_title_from_claude(slug, config).await,
    }
}

pub async fn fetch_content(title: &str, config: &Config) -> Result<Content, Box<dyn std::error::Error>> {
    match config.ai_model {
        AiModel::Gpt4 => fetch_content_from_gpt(title, config).await,
        AiModel::Claude3 | AiModel::Claude4 => fetch_content_from_claude(title, config).await,
    }
}

async fn fetch_title_from_claude(slug: &str, config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    println!("Fetching title from Claude for slug: {}", slug);
    fetch_from_claude(get_title_messages(slug), config).await
}

async fn fetch_from_claude(messages: Vec<Message>, config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    let anthropy_api_key = config.get_api_key()?;
    let url = "https://api.anthropic.com/v1/messages";

    let model = config.ai_model.api_model();

    let headers = build_anthropic_headers(anthropy_api_key)?;
    let body = RequestBody {
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
            Err("Error fetching title from Claude".into())
        }
        Ok(response) => Ok(response.content[0].text.clone()),
    }
}

async fn fetch_title_from_gpt(slug: &str, config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    println!("Fetching title from GPT for slug: {}", slug);
    fetch_from_gpt(get_title_messages(slug), config).await
}

async fn fetch_from_gpt(messages: Vec<Message>, config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    let openai_api_key = config.get_api_key()?;
    let model = config.ai_model.api_model();
    let url = "https://api.openai.com/v1/chat/completions";

    let headers = build_gpt_headers(openai_api_key)?;
    let body = RequestBody {
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
            Err("Error fetching title from OpenAI".into())
        }
        Ok(response) => Ok(response.choices[0].message.content.clone()),
    }
}

async fn fetch_content_from_claude(title: &str, config: &Config) -> Result<Content, Box<dyn std::error::Error>> {
    println!("Fetching content from Claude for title: {}", title);

    let messages = get_messages(title);
    let response = fetch_from_claude(messages, config).await;

    match response {
        Err(_) => {
            println!("Error: {:?}", response);
            Ok(Content {
                title: "".to_string(),
                content: "".to_string(),
            })
        }
        Ok(response) => Ok(Content {
            title: title.to_string(),
            content: response,
        }),
    }
}

async fn fetch_content_from_gpt(title: &str, config: &Config) -> Result<Content, Box<dyn std::error::Error>> {
    println!("Fetching content from GPT for title: {}", title);

    let messages = get_messages(title);
    let response = fetch_from_gpt(messages, config).await;

    match response {
        Err(_) => {
            println!("Error: {:?}", response);
            Ok(Content {
                title: "".to_string(),
                content: "".to_string(),
            })
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

fn build_gpt_headers(api_key: &str) -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", api_key))?,
    );
    Ok(headers)
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

pub fn unslugify(s: &str) -> String {
    s.replace("-", " ")
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

pub fn capitalize_words(s: &str) -> String {
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
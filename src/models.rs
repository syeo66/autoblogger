use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Choice {
    pub index: i64,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GptCompletion {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AnthropicContent {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AnthropicCompletion {
    pub content: Vec<AnthropicContent>,
    pub model: String,
    pub role: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RequestBody {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: i64,
}

#[derive(Debug)]
pub struct Content {
    pub title: String,
    pub content: String,
}
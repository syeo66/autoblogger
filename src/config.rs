use std::env;

#[derive(Debug, Clone)]
pub enum AiModel {
    Gpt4,
    Claude3,
    Claude4,
}

impl AiModel {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "gpt4" => Ok(AiModel::Gpt4),
            "claude3" => Ok(AiModel::Claude3),
            "claude4" => Ok(AiModel::Claude4),
            _ => Err(format!("Invalid AI model: {}. Must be 'gpt4', 'claude3', or 'claude4'", s)),
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            AiModel::Gpt4 => "gpt4",
            AiModel::Claude3 => "claude3",
            AiModel::Claude4 => "claude4",
        }
    }

    pub fn api_model(&self) -> &'static str {
        match self {
            AiModel::Gpt4 => "gpt-4o",
            AiModel::Claude3 => "claude-3-7-sonnet-latest",
            AiModel::Claude4 => "claude-sonnet-4-20250514",
        }
    }

    #[allow(dead_code)]
    pub fn is_claude(&self) -> bool {
        matches!(self, AiModel::Claude3 | AiModel::Claude4)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub ai_model: AiModel,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub db_path: String,
    pub server_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let ai_model_str = env::var("AI_MODEL")
            .map_err(|_| "AI_MODEL environment variable must be set")?;
        
        let ai_model = AiModel::from_str(&ai_model_str)?;

        let openai_api_key = env::var("OPENAI_API_KEY").ok();
        let anthropic_api_key = env::var("ANTHROPIC_API_KEY").ok();

        // Validate that the required API key is present for the selected model
        match ai_model {
            AiModel::Gpt4 => {
                if openai_api_key.is_none() {
                    return Err("OPENAI_API_KEY must be set when using gpt4 model".into());
                }
            }
            AiModel::Claude3 | AiModel::Claude4 => {
                if anthropic_api_key.is_none() {
                    return Err("ANTHROPIC_API_KEY must be set when using claude3 or claude4 model".into());
                }
            }
        }

        let db_path = env::var("DB_PATH").unwrap_or_else(|_| "./blog.db".to_string());
        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .map_err(|_| "SERVER_PORT must be a valid port number")?;

        Ok(Config {
            ai_model,
            openai_api_key,
            anthropic_api_key,
            db_path,
            server_port,
        })
    }

    pub fn get_api_key(&self) -> Result<&str, &'static str> {
        match self.ai_model {
            AiModel::Gpt4 => {
                self.openai_api_key.as_deref()
                    .ok_or("OpenAI API key not configured")
            }
            AiModel::Claude3 | AiModel::Claude4 => {
                self.anthropic_api_key.as_deref()
                    .ok_or("Anthropic API key not configured")
            }
        }
    }
}
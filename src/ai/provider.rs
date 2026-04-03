// src/ai/provider.rs

#[derive(Debug, Clone)]
pub enum Provider {
    OpenAI,
    Claude,
    Gemini,
    Ollama,
}

impl Provider {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Self::OpenAI,
            "claude" => Self::Claude,
            "gemini" => Self::Gemini,
            "ollama" => Self::Ollama,
            _ => Self::OpenAI,
        }
    }
}
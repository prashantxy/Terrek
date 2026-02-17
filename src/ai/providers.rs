pub enum provider {
    OpenAI,
    Claude,
    Gemini,
    Ollama,
}
impl provider{
    pub fn from_str(s: &str)->self{
        match s {
            "openai" => Self::OpenAI,
            "claude" => Self::Claude,
            "gemini" => Self:: Gemini,
            "ollama" => Self::Ollama,
            _Self::OpenAI
        }
    }
}
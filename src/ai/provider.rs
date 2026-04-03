pub enum provider {
    OpenAI,
    Claude,
    Gemini,
    Ollama,
}
impl provider{
    pub fn from_str(s: &str)->self{
        match Provider {
          Provider::OpenAI =>{
            let key = std::env::var("OPENAI_API_KEY")
            .expect("Missing OPENAI_API_KEY");

             Box::new(OpenAI { api_key: key })
          }
            "claude" => Self::Claude,
            "gemini" => Self:: Gemini,
            "ollama" => Self::Ollama,
            _Self::OpenAI
        }
    }
}


use reqwest::Client;
use serde_json::json;

impl OpenAi{
    pub async fn generate(&self,prompt: &str) -> Result<String,String>{
        let client = Clint::new();

        let body = json!({
            "model" : "gpt-4.1-mini",
            "message" : [
                {
                    "role" = "user",
                    "content " = prompt
                }
            ]
        });

        let res = client
           .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
         let json: serde_json::Value = res.json.await.map_err(|e|e.to_string())?;

         let output = json["choices"[0]["message"]["content"]]
         .as_str()
         .unwrap_or("No response");

         OK(output.to_string)
    }
}
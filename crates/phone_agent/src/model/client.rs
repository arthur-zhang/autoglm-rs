//! Model client for AI inference using OpenAI-compatible API

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContent, ChatCompletionRequestUserMessageContentPart,
        CreateChatCompletionRequestArgs, ImageDetail, ImageUrl,
    },
    Client,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, Write};
use std::time::Instant;

use crate::config::{get_message, Language};

/// Configuration for the AI model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub frequency_penalty: f32,
    pub lang: Language,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8000/v1".to_string(),
            api_key: "EMPTY".to_string(),
            model_name: "autoglm-phone-9b".to_string(),
            max_tokens: 3000,
            temperature: 0.0,
            top_p: 0.85,
            frequency_penalty: 0.2,
            lang: Language::Chinese,
        }
    }
}

impl ModelConfig {
    /// Create a new ModelConfig with custom settings
    pub fn new(base_url: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model_name: model_name.into(),
            ..Default::default()
        }
    }

    /// Set the API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = api_key.into();
        self
    }

    /// Set the language
    pub fn with_lang(mut self, lang: Language) -> Self {
        self.lang = lang;
        self
    }
}

/// Response from the AI model
#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub thinking: String,
    pub action: String,
    pub raw_content: String,
    /// Time to first token (seconds)
    pub time_to_first_token: Option<f64>,
    /// Time to thinking end (seconds)
    pub time_to_thinking_end: Option<f64>,
    /// Total inference time (seconds)
    pub total_time: Option<f64>,
}

/// Client for interacting with OpenAI-compatible vision-language models
pub struct ModelClient {
    config: ModelConfig,
    client: Client<OpenAIConfig>,
}

impl ModelClient {
    /// Create a new ModelClient
    pub fn new(config: ModelConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_base(&config.base_url)
            .with_api_key(&config.api_key);

        let client = Client::with_config(openai_config);

        Self { config, client }
    }

    /// Test connection to the model API by sending a simple request
    pub async fn test_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model_name)
            .max_tokens(5_u32)
            .temperature(0.0_f32)
            .messages(vec![ChatCompletionRequestUserMessageArgs::default()
                .content("Hi")
                .build()?
                .into()])
            .build()?;

        let response = self.client.chat().create(request).await?;

        // Check if we got a valid response
        if response.choices.is_empty() {
            return Err("Received empty response from API".into());
        }

        Ok(())
    }

    /// Send a request to the model
    pub async fn request(
        &self,
        messages: Vec<ChatCompletionRequestMessage>,
    ) -> Result<ModelResponse, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let mut time_to_first_token: Option<f64> = None;
        let mut time_to_thinking_end: Option<f64> = None;

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model_name)
            .max_tokens(self.config.max_tokens)
            .temperature(self.config.temperature)
            .top_p(self.config.top_p)
            .frequency_penalty(self.config.frequency_penalty)
            .messages(messages)
            .stream(true)
            .build()?;

        let mut stream = self.client.chat().create_stream(request).await?;

        let mut raw_content = String::new();
        let mut buffer = String::new();
        let action_markers = ["finish(message=", "do(action="];
        let mut in_action_phase = false;
        let mut first_token_received = false;

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    for choice in response.choices {
                        if let Some(content) = choice.delta.content {
                            raw_content.push_str(&content);

                            // Record time to first token
                            if !first_token_received {
                                time_to_first_token = Some(start_time.elapsed().as_secs_f64());
                                first_token_received = true;
                            }

                            if in_action_phase {
                                continue;
                            }

                            buffer.push_str(&content);

                            // Check if any marker is fully present in buffer
                            let mut marker_found = false;
                            for marker in &action_markers {
                                if buffer.contains(marker) {
                                    // Marker found, print everything before it
                                    let parts: Vec<&str> = buffer.splitn(2, marker).collect();
                                    print!("{}", parts[0]);
                                    println!();
                                    io::stdout().flush().ok();
                                    in_action_phase = true;
                                    marker_found = true;

                                    // Record time to thinking end
                                    if time_to_thinking_end.is_none() {
                                        time_to_thinking_end =
                                            Some(start_time.elapsed().as_secs_f64());
                                    }

                                    break;
                                }
                            }

                            if marker_found {
                                continue;
                            }

                            // Check if buffer ends with a prefix of any marker
                            let mut is_potential_marker = false;
                            for marker in &action_markers {
                                for i in 1..marker.len() {
                                    if buffer.ends_with(&marker[..i]) {
                                        is_potential_marker = true;
                                        break;
                                    }
                                }
                                if is_potential_marker {
                                    break;
                                }
                            }

                            if !is_potential_marker {
                                print!("{}", buffer);
                                io::stdout().flush().ok();
                                buffer.clear();
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(Box::new(e));
                }
            }
        }

        let total_time = start_time.elapsed().as_secs_f64();

        // Parse thinking and action from response
        let (thinking, action) = self.parse_response(&raw_content);

        // Print performance metrics
        let lang = self.config.lang;
        println!();
        println!("{}", "=".repeat(50));
        println!("⏱️  {}:", get_message("performance_metrics", lang));
        println!("{}", "-".repeat(50));
        if let Some(ttft) = time_to_first_token {
            println!("{}: {:.3}s", get_message("time_to_first_token", lang), ttft);
        }
        if let Some(ttte) = time_to_thinking_end {
            println!(
                "{}:        {:.3}s",
                get_message("time_to_thinking_end", lang),
                ttte
            );
        }
        println!(
            "{}:          {:.3}s",
            get_message("total_inference_time", lang),
            total_time
        );
        println!("{}", "=".repeat(50));

        Ok(ModelResponse {
            thinking,
            action,
            raw_content,
            time_to_first_token,
            time_to_thinking_end,
            total_time: Some(total_time),
        })
    }

    /// Parse the model response into thinking and action parts
    fn parse_response(&self, content: &str) -> (String, String) {
        // Rule 1: Check for finish(message=
        if content.contains("finish(message=") {
            let parts: Vec<&str> = content.splitn(2, "finish(message=").collect();
            let thinking = parts[0].trim().to_string();
            let action = format!("finish(message={}", parts[1]);
            return (thinking, action);
        }

        // Rule 2: Check for do(action=
        if content.contains("do(action=") {
            let parts: Vec<&str> = content.splitn(2, "do(action=").collect();
            let thinking = parts[0].trim().to_string();
            let action = format!("do(action={}", parts[1]);
            return (thinking, action);
        }

        // Rule 3: Fallback to legacy XML tag parsing
        if content.contains("<answer>") {
            let parts: Vec<&str> = content.splitn(2, "<answer>").collect();
            let thinking = parts[0]
                .replace("<think>", "")
                .replace("</think>", "")
                .trim()
                .to_string();
            let action = parts[1].replace("</answer>", "").trim().to_string();
            return (thinking, action);
        }

        // Rule 4: No markers found, return content as action
        (String::new(), content.to_string())
    }
}

/// Helper for building conversation messages
pub struct MessageBuilder;

impl MessageBuilder {
    /// Create a system message
    pub fn create_system_message(content: &str) -> ChatCompletionRequestMessage {
        ChatCompletionRequestSystemMessageArgs::default()
            .content(content)
            .build()
            .unwrap()
            .into()
    }

    /// Create a user message with optional image
    pub fn create_user_message(
        text: &str,
        image_base64: Option<&str>,
    ) -> ChatCompletionRequestMessage {
        let mut content_parts: Vec<ChatCompletionRequestUserMessageContentPart> = Vec::new();

        if let Some(img) = image_base64 {
            content_parts.push(ChatCompletionRequestUserMessageContentPart::ImageUrl(
                async_openai::types::ChatCompletionRequestMessageContentPartImage {
                    image_url: ImageUrl {
                        url: format!("data:image/png;base64,{}", img),
                        detail: Some(ImageDetail::Auto),
                    },
                },
            ));
        }

        content_parts.push(ChatCompletionRequestUserMessageContentPart::Text(
            async_openai::types::ChatCompletionRequestMessageContentPartText {
                text: text.to_string(),
            },
        ));

        ChatCompletionRequestUserMessageArgs::default()
            .content(ChatCompletionRequestUserMessageContent::Array(content_parts))
            .build()
            .unwrap()
            .into()
    }

    /// Create an assistant message
    pub fn create_assistant_message(content: &str) -> ChatCompletionRequestMessage {
        ChatCompletionRequestAssistantMessageArgs::default()
            .content(content)
            .build()
            .unwrap()
            .into()
    }

    /// Remove image content from a message to save context space
    pub fn remove_images_from_message(
        message: ChatCompletionRequestMessage,
    ) -> ChatCompletionRequestMessage {
        match message {
            ChatCompletionRequestMessage::User(mut user_msg) => {
                if let ChatCompletionRequestUserMessageContent::Array(parts) = &user_msg.content {
                    let text_parts: Vec<_> = parts
                        .iter()
                        .filter(|p| {
                            matches!(p, ChatCompletionRequestUserMessageContentPart::Text(_))
                        })
                        .cloned()
                        .collect();
                    user_msg.content = ChatCompletionRequestUserMessageContent::Array(text_parts);
                }
                ChatCompletionRequestMessage::User(user_msg)
            }
            other => other,
        }
    }

    /// Build screen info string for the model
    pub fn build_screen_info(current_app: &str) -> String {
        json!({
            "current_app": current_app
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert_eq!(config.base_url, "http://localhost:8000/v1");
        assert_eq!(config.model_name, "autoglm-phone-9b");
    }

    #[test]
    fn test_model_config_builder() {
        let config = ModelConfig::new("http://custom:8080", "custom-model")
            .with_api_key("test-key")
            .with_lang(Language::English);

        assert_eq!(config.base_url, "http://custom:8080");
        assert_eq!(config.model_name, "custom-model");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.lang, Language::English);
    }

    #[test]
    fn test_build_screen_info() {
        let info = MessageBuilder::build_screen_info("WeChat");
        assert!(info.contains("WeChat"));
        assert!(info.contains("current_app"));
    }
}

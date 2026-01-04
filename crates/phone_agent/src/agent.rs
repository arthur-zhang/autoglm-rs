//! Main PhoneAgent class for orchestrating phone automation

use async_openai::types::ChatCompletionRequestMessage;
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::actions::{
    finish_action, parse_action, ActionHandler, ConfirmationCallback, TakeoverCallback,
};
use crate::config::{get_messages, get_system_prompt, Language};
use crate::device_factory::get_device_factory;
use crate::error::Result;
use crate::model::{MessageBuilder, ModelClient, ModelConfig};
use crate::screenshot_saver::ScreenshotSaver;

/// Configuration for the PhoneAgent
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_steps: usize,
    pub device_id: Option<String>,
    pub lang: Language,
    pub system_prompt: Option<String>,
    pub verbose: bool,
    /// Directory to save screenshots (if set, screenshots will be saved to disk)
    pub screenshot_dir: Option<PathBuf>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 100,
            device_id: None,
            lang: Language::Chinese,
            system_prompt: None,
            verbose: true,
            screenshot_dir: None,
        }
    }
}

impl AgentConfig {
    /// Create a new AgentConfig
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max steps
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Set device ID
    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    /// Set language
    pub fn with_lang(mut self, lang: Language) -> Self {
        self.lang = lang;
        self
    }

    /// Set custom system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set verbose mode
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set screenshot directory
    pub fn with_screenshot_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.screenshot_dir = Some(dir.into());
        self
    }

    /// Get the system prompt (custom or default based on language)
    pub fn get_system_prompt(&self) -> String {
        self.system_prompt
            .clone()
            .unwrap_or_else(|| get_system_prompt(self.lang))
    }
}

/// Result of a single agent step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub success: bool,
    pub finished: bool,
    pub action: Option<HashMap<String, serde_json::Value>>,
    pub thinking: String,
    pub message: Option<String>,
}

/// AI-powered agent for automating Android phone interactions
///
/// The agent uses a vision-language model to understand screen content
/// and decide on actions to complete user tasks.
pub struct PhoneAgent {
    model_config: ModelConfig,
    agent_config: AgentConfig,
    model_client: ModelClient,
    action_handler: ActionHandler,
    context: Vec<ChatCompletionRequestMessage>,
    step_count: usize,
    screenshot_saver: Option<ScreenshotSaver>,
}

impl PhoneAgent {
    /// Create a new PhoneAgent
    ///
    /// # Arguments
    /// * `model_config` - Configuration for the AI model
    /// * `agent_config` - Configuration for the agent behavior
    /// * `confirmation_callback` - Optional callback for sensitive action confirmation
    /// * `takeover_callback` - Optional callback for takeover requests
    pub async fn new(
        model_config: Option<ModelConfig>,
        agent_config: Option<AgentConfig>,
        confirmation_callback: Option<ConfirmationCallback>,
        takeover_callback: Option<TakeoverCallback>,
    ) -> Result<Self> {
        let model_config = model_config.unwrap_or_default();
        let agent_config = agent_config.unwrap_or_default();

        let model_client = ModelClient::new(model_config.clone());
        let action_handler = ActionHandler::new(
            agent_config.device_id.clone(),
            confirmation_callback,
            takeover_callback,
        );

        // Initialize screenshot saver if directory is configured
        let screenshot_saver = if let Some(ref dir) = agent_config.screenshot_dir {
            Some(ScreenshotSaver::new(dir).await?)
        } else {
            None
        };

        Ok(Self {
            model_config,
            agent_config,
            model_client,
            action_handler,
            context: Vec::new(),
            step_count: 0,
            screenshot_saver,
        })
    }

    /// Run the agent to complete a task
    ///
    /// # Arguments
    /// * `task` - Natural language description of the task
    ///
    /// # Returns
    /// Final message from the agent
    pub async fn run(&mut self, task: &str) -> Result<String> {
        self.context.clear();
        self.step_count = 0;

        // First step with user prompt
        let result = self.execute_step(Some(task), true).await?;

        if result.finished {
            return Ok(result.message.unwrap_or_else(|| "Task completed".to_string()));
        }

        // Continue until finished or max steps reached
        while self.step_count < self.agent_config.max_steps {
            let result = self.execute_step(None, false).await?;

            if result.finished {
                return Ok(result.message.unwrap_or_else(|| "Task completed".to_string()));
            }
        }

        Ok("Max steps reached".to_string())
    }

    /// Execute a single step of the agent
    ///
    /// Useful for manual control or debugging.
    ///
    /// # Arguments
    /// * `task` - Task description (only needed for first step)
    ///
    /// # Returns
    /// StepResult with step details
    pub async fn step(&mut self, task: Option<&str>) -> Result<StepResult> {
        let is_first = self.context.is_empty();

        if is_first && task.is_none() {
            return Err(crate::error::AdbError::CommandFailed(
                "Task is required for the first step".to_string(),
            ));
        }

        self.execute_step(task, is_first).await
    }

    /// Reset the agent state for a new task
    pub async fn reset(&mut self) {
        self.context.clear();
        self.step_count = 0;

        // Create a new session directory for screenshots in interactive mode
        if let Some(ref mut saver) = self.screenshot_saver {
            if let Err(e) = saver.new_session().await {
                eprintln!("Warning: Failed to create new screenshot session: {}", e);
            }
        }
    }

    /// Execute a single step of the agent loop
    async fn execute_step(
        &mut self,
        user_prompt: Option<&str>,
        is_first: bool,
    ) -> Result<StepResult> {
        self.step_count += 1;

        // Capture current screen state
        let factory = get_device_factory().read().await;
        let screenshot = factory
            .get_screenshot(self.agent_config.device_id.as_deref(), 10)
            .await?;
        let current_app = factory
            .get_current_app(self.agent_config.device_id.as_deref())
            .await?;
        drop(factory);

        // Save screenshot to disk if configured
        if let Some(ref mut saver) = self.screenshot_saver {
            if let Err(e) = saver.save(&screenshot.base64_data).await {
                eprintln!("Warning: Failed to save screenshot: {}", e);
            }
        }

        // Build messages
        if is_first {
            self.context.push(MessageBuilder::create_system_message(
                &self.agent_config.get_system_prompt(),
            ));

            let screen_info = MessageBuilder::build_screen_info(&current_app);
            let text_content = format!("{}\n\n{}", user_prompt.unwrap_or(""), screen_info);

            self.context.push(MessageBuilder::create_user_message(
                &text_content,
                Some(&screenshot.base64_data),
            ));
        } else {
            let screen_info = MessageBuilder::build_screen_info(&current_app);
            let text_content = format!("** Screen Info **\n\n{}", screen_info);

            self.context.push(MessageBuilder::create_user_message(
                &text_content,
                Some(&screenshot.base64_data),
            ));
        }

        // Get model response
        let msgs = get_messages(self.agent_config.lang);
        if self.agent_config.verbose {
            println!("\n{}", "=".repeat(50));
            println!(
                "\u{1F4AD} {}:",
                msgs.get("thinking").copied().unwrap_or("Thinking")
            );
            println!("{}", "-".repeat(50));
        }

        let response = match self.model_client.request(self.context.clone()).await {
            Ok(r) => r,
            Err(e) => {
                if self.agent_config.verbose {
                    eprintln!("Model error: {}", e);
                }
                return Ok(StepResult {
                    success: false,
                    finished: true,
                    action: None,
                    thinking: String::new(),
                    message: Some(format!("Model error: {}", e)),
                });
            }
        };

        // Parse action from response
        let action = match parse_action(&response.action) {
            Ok(a) => a,
            Err(_) => {
                if self.agent_config.verbose {
                    eprintln!("Failed to parse action, treating as finish");
                }
                finish_action(Some(&response.action))
            }
        };

        if self.agent_config.verbose {
            println!("{}", "-".repeat(50));
            println!(
                "\u{1F3AF} {}:",
                msgs.get("action").copied().unwrap_or("Action")
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&action).unwrap_or_else(|_| format!("{:?}", action))
            );
            println!("{}\n", "=".repeat(50));
        }

        // Remove image from context to save space
        if let Some(last) = self.context.pop() {
            self.context
                .push(MessageBuilder::remove_images_from_message(last));
        }

        // Execute action
        let result = self
            .action_handler
            .execute(&action, screenshot.width, screenshot.height)
            .await;

        // Add assistant response to context
        self.context.push(MessageBuilder::create_assistant_message(
            &format!(
                "<think>{}</think><answer>{}</answer>",
                response.thinking, response.action
            ),
        ));

        // Check if finished
        let finished = action.get("_metadata").and_then(|v| v.as_str()) == Some("finish")
            || result.should_finish;

        if finished && self.agent_config.verbose {
            let action_msg = action
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let display_msg = result
                .message
                .as_ref()
                .or(action_msg.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(msgs.get("done").copied().unwrap_or("Done"));

            println!("\n\u{1F389} {}", "=".repeat(48));
            println!(
                "\u{2705} {}: {}",
                msgs.get("task_completed").copied().unwrap_or("Task Completed"),
                display_msg
            );
            println!("{}\n", "=".repeat(50));
        }

        Ok(StepResult {
            success: result.success,
            finished,
            action: Some(action.clone()),
            thinking: response.thinking,
            message: result.message.or_else(|| {
                action
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }),
        })
    }

    /// Get the current conversation context
    pub fn context(&self) -> &[ChatCompletionRequestMessage] {
        &self.context
    }

    /// Get the current step count
    pub fn step_count(&self) -> usize {
        self.step_count
    }

    /// Get the model config
    pub fn model_config(&self) -> &ModelConfig {
        &self.model_config
    }

    /// Get the agent config
    pub fn agent_config(&self) -> &AgentConfig {
        &self.agent_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_steps, 100);
        assert_eq!(config.lang, Language::Chinese);
        assert!(config.verbose);
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new()
            .with_max_steps(50)
            .with_device_id("device123")
            .with_lang(Language::English)
            .with_verbose(false);

        assert_eq!(config.max_steps, 50);
        assert_eq!(config.device_id, Some("device123".to_string()));
        assert_eq!(config.lang, Language::English);
        assert!(!config.verbose);
    }

    #[test]
    fn test_step_result() {
        let result = StepResult {
            success: true,
            finished: false,
            action: None,
            thinking: "Test thinking".to_string(),
            message: Some("Test message".to_string()),
        };

        assert!(result.success);
        assert!(!result.finished);
        assert_eq!(result.thinking, "Test thinking");
    }
}

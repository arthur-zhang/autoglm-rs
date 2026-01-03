//! Action handler for processing AI model outputs

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Duration;
use tokio::time::sleep;

use crate::config::TIMING_CONFIG;
use crate::device_factory::get_device_factory;
use crate::error::{AdbError, Result};

/// Result of an action execution
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub success: bool,
    pub should_finish: bool,
    pub message: Option<String>,
    pub requires_confirmation: bool,
}

impl ActionResult {
    /// Create a successful result
    pub fn success() -> Self {
        Self {
            success: true,
            should_finish: false,
            message: None,
            requires_confirmation: false,
        }
    }

    /// Create a failure result
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            should_finish: false,
            message: Some(message.into()),
            requires_confirmation: false,
        }
    }

    /// Create a finish result
    pub fn finish(message: Option<String>) -> Self {
        Self {
            success: true,
            should_finish: true,
            message,
            requires_confirmation: false,
        }
    }
}

/// Callback type for confirmation
pub type ConfirmationCallback = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// Callback type for takeover
pub type TakeoverCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Handles execution of actions from AI model output
pub struct ActionHandler {
    device_id: Option<String>,
    confirmation_callback: ConfirmationCallback,
    takeover_callback: TakeoverCallback,
}

impl ActionHandler {
    /// Create a new ActionHandler
    pub fn new(
        device_id: Option<String>,
        confirmation_callback: Option<ConfirmationCallback>,
        takeover_callback: Option<TakeoverCallback>,
    ) -> Self {
        Self {
            device_id,
            confirmation_callback: confirmation_callback
                .unwrap_or_else(|| Box::new(default_confirmation)),
            takeover_callback: takeover_callback.unwrap_or_else(|| Box::new(default_takeover)),
        }
    }

    /// Execute an action from the AI model
    pub async fn execute(
        &self,
        action: &HashMap<String, Value>,
        screen_width: u32,
        screen_height: u32,
    ) -> ActionResult {
        let action_type = action
            .get("_metadata")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if action_type == "finish" {
            return ActionResult::finish(
                action
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            );
        }

        if action_type != "do" {
            return ActionResult::failure(format!("Unknown action type: {}", action_type));
        }

        let action_name = action
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let result = match action_name {
            "Launch" => self.handle_launch(action).await,
            "Tap" => self.handle_tap(action, screen_width, screen_height).await,
            "Type" | "Type_Name" => self.handle_type(action).await,
            "Swipe" => self.handle_swipe(action, screen_width, screen_height).await,
            "Back" => self.handle_back().await,
            "Home" => self.handle_home().await,
            "Double Tap" => {
                self.handle_double_tap(action, screen_width, screen_height)
                    .await
            }
            "Long Press" => {
                self.handle_long_press(action, screen_width, screen_height)
                    .await
            }
            "Wait" => self.handle_wait(action).await,
            "Take_over" => self.handle_takeover(action),
            "Note" => Ok(ActionResult::success()),
            "Call_API" => Ok(ActionResult::success()),
            "Interact" => Ok(ActionResult {
                success: true,
                should_finish: false,
                message: Some("User interaction required".to_string()),
                requires_confirmation: false,
            }),
            _ => Err(AdbError::CommandFailed(format!(
                "Unknown action: {}",
                action_name
            ))),
        };

        match result {
            Ok(r) => r,
            Err(e) => ActionResult::failure(format!("Action failed: {}", e)),
        }
    }

    /// Convert relative coordinates (0-1000) to absolute pixels
    fn convert_relative_to_absolute(
        &self,
        element: &[i64],
        screen_width: u32,
        screen_height: u32,
    ) -> (i32, i32) {
        let x = (element[0] as f64 / 1000.0 * screen_width as f64) as i32;
        let y = (element[1] as f64 / 1000.0 * screen_height as f64) as i32;
        (x, y)
    }

    async fn handle_launch(&self, action: &HashMap<String, Value>) -> Result<ActionResult> {
        let app_name = action
            .get("app")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AdbError::CommandFailed("No app name specified".to_string()))?;

        let factory = get_device_factory().read().await;
        let success = factory
            .launch_app(app_name, self.device_id.as_deref(), None)
            .await?;

        if success {
            Ok(ActionResult::success())
        } else {
            Ok(ActionResult::failure(format!("App not found: {}", app_name)))
        }
    }

    async fn handle_tap(
        &self,
        action: &HashMap<String, Value>,
        width: u32,
        height: u32,
    ) -> Result<ActionResult> {
        let element = action
            .get("element")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdbError::CommandFailed("No element coordinates".to_string()))?;

        let coords: Vec<i64> = element.iter().filter_map(|v| v.as_i64()).collect();

        if coords.len() < 2 {
            return Err(AdbError::CommandFailed(
                "Invalid element coordinates".to_string(),
            ));
        }

        let (x, y) = self.convert_relative_to_absolute(&coords, width, height);

        // Check for sensitive operation
        if let Some(message) = action.get("message").and_then(|v| v.as_str()) {
            if !(self.confirmation_callback)(message) {
                return Ok(ActionResult {
                    success: false,
                    should_finish: true,
                    message: Some("User cancelled sensitive operation".to_string()),
                    requires_confirmation: false,
                });
            }
        }

        let factory = get_device_factory().read().await;
        factory.tap(x, y, self.device_id.as_deref(), None).await?;

        Ok(ActionResult::success())
    }

    async fn handle_type(&self, action: &HashMap<String, Value>) -> Result<ActionResult> {
        let text = action
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let factory = get_device_factory().read().await;

        // Switch to ADB keyboard
        let original_ime = factory
            .detect_and_set_adb_keyboard(self.device_id.as_deref())
            .await?;
        sleep(Duration::from_secs_f64(
            TIMING_CONFIG.action.keyboard_switch_delay,
        ))
        .await;

        // Clear existing text and type new text
        factory.clear_text(self.device_id.as_deref()).await?;
        sleep(Duration::from_secs_f64(TIMING_CONFIG.action.text_clear_delay)).await;

        // Type text
        factory.type_text(text, self.device_id.as_deref()).await?;
        sleep(Duration::from_secs_f64(TIMING_CONFIG.action.text_input_delay)).await;

        // Restore original keyboard
        factory
            .restore_keyboard(&original_ime, self.device_id.as_deref())
            .await?;
        sleep(Duration::from_secs_f64(
            TIMING_CONFIG.action.keyboard_restore_delay,
        ))
        .await;

        Ok(ActionResult::success())
    }

    async fn handle_swipe(
        &self,
        action: &HashMap<String, Value>,
        width: u32,
        height: u32,
    ) -> Result<ActionResult> {
        let start = action
            .get("start")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdbError::CommandFailed("Missing start coordinates".to_string()))?;

        let end = action
            .get("end")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdbError::CommandFailed("Missing end coordinates".to_string()))?;

        let start_coords: Vec<i64> = start.iter().filter_map(|v| v.as_i64()).collect();
        let end_coords: Vec<i64> = end.iter().filter_map(|v| v.as_i64()).collect();

        if start_coords.len() < 2 || end_coords.len() < 2 {
            return Err(AdbError::CommandFailed(
                "Invalid swipe coordinates".to_string(),
            ));
        }

        let (start_x, start_y) = self.convert_relative_to_absolute(&start_coords, width, height);
        let (end_x, end_y) = self.convert_relative_to_absolute(&end_coords, width, height);

        let factory = get_device_factory().read().await;
        factory
            .swipe(
                start_x,
                start_y,
                end_x,
                end_y,
                None,
                self.device_id.as_deref(),
                None,
            )
            .await?;

        Ok(ActionResult::success())
    }

    async fn handle_back(&self) -> Result<ActionResult> {
        let factory = get_device_factory().read().await;
        factory.back(self.device_id.as_deref(), None).await?;
        Ok(ActionResult::success())
    }

    async fn handle_home(&self) -> Result<ActionResult> {
        let factory = get_device_factory().read().await;
        factory.home(self.device_id.as_deref(), None).await?;
        Ok(ActionResult::success())
    }

    async fn handle_double_tap(
        &self,
        action: &HashMap<String, Value>,
        width: u32,
        height: u32,
    ) -> Result<ActionResult> {
        let element = action
            .get("element")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdbError::CommandFailed("No element coordinates".to_string()))?;

        let coords: Vec<i64> = element.iter().filter_map(|v| v.as_i64()).collect();

        if coords.len() < 2 {
            return Err(AdbError::CommandFailed(
                "Invalid element coordinates".to_string(),
            ));
        }

        let (x, y) = self.convert_relative_to_absolute(&coords, width, height);

        let factory = get_device_factory().read().await;
        factory
            .double_tap(x, y, self.device_id.as_deref(), None)
            .await?;

        Ok(ActionResult::success())
    }

    async fn handle_long_press(
        &self,
        action: &HashMap<String, Value>,
        width: u32,
        height: u32,
    ) -> Result<ActionResult> {
        let element = action
            .get("element")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AdbError::CommandFailed("No element coordinates".to_string()))?;

        let coords: Vec<i64> = element.iter().filter_map(|v| v.as_i64()).collect();

        if coords.len() < 2 {
            return Err(AdbError::CommandFailed(
                "Invalid element coordinates".to_string(),
            ));
        }

        let (x, y) = self.convert_relative_to_absolute(&coords, width, height);

        let factory = get_device_factory().read().await;
        factory
            .long_press(x, y, 3000, self.device_id.as_deref(), None)
            .await?;

        Ok(ActionResult::success())
    }

    async fn handle_wait(&self, action: &HashMap<String, Value>) -> Result<ActionResult> {
        let duration_str = action
            .get("duration")
            .and_then(|v| v.as_str())
            .unwrap_or("1 seconds");

        // Parse duration from string like "1 seconds" or "2 seconds"
        let duration: f64 = duration_str
            .replace("seconds", "")
            .replace("second", "")
            .trim()
            .parse()
            .unwrap_or(1.0);

        sleep(Duration::from_secs_f64(duration)).await;
        Ok(ActionResult::success())
    }

    fn handle_takeover(&self, action: &HashMap<String, Value>) -> Result<ActionResult> {
        let message = action
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("User intervention required");

        (self.takeover_callback)(message);
        Ok(ActionResult::success())
    }
}

/// Default confirmation callback using console input
fn default_confirmation(message: &str) -> bool {
    print!("Sensitive operation: {}\nConfirm? (Y/N): ", message);
    io::stdout().flush().ok();

    let mut response = String::new();
    io::stdin().read_line(&mut response).ok();
    response.trim().to_uppercase() == "Y"
}

/// Default takeover callback using console input
fn default_takeover(message: &str) {
    println!("{}", message);
    print!("Press Enter after completing manual operation...");
    io::stdout().flush().ok();

    let mut response = String::new();
    io::stdin().read_line(&mut response).ok();
}

/// Parse action from model response
///
/// Returns a HashMap representing the parsed action.
pub fn parse_action(response: &str) -> std::result::Result<HashMap<String, Value>, String> {
    let response = response.trim();
    println!("Parsing action: {}", response);

    // Handle Type action with special text parsing
    if response.starts_with("do(action=\"Type\"") || response.starts_with("do(action=\"Type_Name\"")
    {
        if let Some(text_start) = response.find("text=") {
            let text_part = &response[text_start + 6..]; // Skip 'text="'
            if let Some(end_pos) = text_part.rfind("\")") {
                let text = &text_part[..end_pos];
                let mut action = HashMap::new();
                action.insert("_metadata".to_string(), json!("do"));
                action.insert("action".to_string(), json!("Type"));
                action.insert("text".to_string(), json!(text));
                return Ok(action);
            }
        }
    }

    // Handle do() actions
    if response.starts_with("do(") {
        return parse_do_action(response);
    }

    // Handle finish() actions
    if response.starts_with("finish(") {
        let message = response
            .replace("finish(message=", "")
            .trim_start_matches('"')
            .trim_end_matches("\")")
            .to_string();

        let mut action = HashMap::new();
        action.insert("_metadata".to_string(), json!("finish"));
        action.insert("message".to_string(), json!(message));
        return Ok(action);
    }

    Err(format!("Failed to parse action: {}", response))
}

/// Parse a do() action string into a HashMap
fn parse_do_action(response: &str) -> std::result::Result<HashMap<String, Value>, String> {
    let mut action = HashMap::new();
    action.insert("_metadata".to_string(), json!("do"));

    // Remove "do(" prefix and ")" suffix
    let inner = response
        .strip_prefix("do(")
        .and_then(|s| s.strip_suffix(")"))
        .ok_or_else(|| "Invalid do() format".to_string())?;

    // Parse key=value pairs
    // This is a simplified parser that handles the common cases
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_string = false;
    let mut in_array = false;
    let mut escape_next = false;
    let mut parsing_value = false;

    for ch in inner.chars() {
        if escape_next {
            if parsing_value {
                current_value.push(ch);
            }
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => {
                escape_next = true;
                if parsing_value {
                    current_value.push(ch);
                }
            }
            '"' if !in_array => {
                in_string = !in_string;
                if parsing_value {
                    current_value.push(ch);
                }
            }
            '[' if !in_string => {
                in_array = true;
                if parsing_value {
                    current_value.push(ch);
                }
            }
            ']' if !in_string => {
                in_array = false;
                if parsing_value {
                    current_value.push(ch);
                }
            }
            '=' if !in_string && !in_array && !parsing_value => {
                parsing_value = true;
            }
            ',' if !in_string && !in_array => {
                // End of key=value pair
                if !current_key.is_empty() {
                    let value = parse_value(current_value.trim());
                    action.insert(current_key.trim().to_string(), value);
                }
                current_key.clear();
                current_value.clear();
                parsing_value = false;
            }
            _ => {
                if parsing_value {
                    current_value.push(ch);
                } else {
                    current_key.push(ch);
                }
            }
        }
    }

    // Handle last key=value pair
    if !current_key.is_empty() {
        let value = parse_value(current_value.trim());
        action.insert(current_key.trim().to_string(), value);
    }

    Ok(action)
}

/// Parse a value string into a serde_json Value
fn parse_value(s: &str) -> Value {
    let s = s.trim();

    // String value
    if s.starts_with('"') && s.ends_with('"') {
        return json!(s[1..s.len() - 1].replace("\\n", "\n").replace("\\t", "\t"));
    }

    // Array value
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        let elements: Vec<Value> = inner
            .split(',')
            .map(|e| {
                let e = e.trim();
                if let Ok(n) = e.parse::<i64>() {
                    json!(n)
                } else if let Ok(f) = e.parse::<f64>() {
                    json!(f)
                } else {
                    json!(e.trim_matches('"'))
                }
            })
            .collect();
        return json!(elements);
    }

    // Number value
    if let Ok(n) = s.parse::<i64>() {
        return json!(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return json!(f);
    }

    // Boolean
    if s == "true" || s == "True" {
        return json!(true);
    }
    if s == "false" || s == "False" {
        return json!(false);
    }

    // Default to string
    json!(s)
}

/// Helper function for creating 'do' actions
pub fn do_action(action_name: &str) -> HashMap<String, Value> {
    let mut action = HashMap::new();
    action.insert("_metadata".to_string(), json!("do"));
    action.insert("action".to_string(), json!(action_name));
    action
}

/// Helper function for creating 'finish' actions
pub fn finish_action(message: Option<&str>) -> HashMap<String, Value> {
    let mut action = HashMap::new();
    action.insert("_metadata".to_string(), json!("finish"));
    if let Some(msg) = message {
        action.insert("message".to_string(), json!(msg));
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action_tap() {
        let result = parse_action("do(action=\"Tap\", element=[500, 300])").unwrap();
        assert_eq!(result.get("_metadata").unwrap(), "do");
        assert_eq!(result.get("action").unwrap(), "Tap");
    }

    #[test]
    fn test_parse_action_type() {
        let result = parse_action("do(action=\"Type\", text=\"Hello World\")").unwrap();
        assert_eq!(result.get("_metadata").unwrap(), "do");
        assert_eq!(result.get("action").unwrap(), "Type");
        assert_eq!(result.get("text").unwrap(), "Hello World");
    }

    #[test]
    fn test_parse_action_finish() {
        let result = parse_action("finish(message=\"Task completed\")").unwrap();
        assert_eq!(result.get("_metadata").unwrap(), "finish");
        assert_eq!(result.get("message").unwrap(), "Task completed");
    }

    #[test]
    fn test_parse_action_swipe() {
        let result = parse_action("do(action=\"Swipe\", start=[100, 500], end=[100, 200])").unwrap();
        assert_eq!(result.get("_metadata").unwrap(), "do");
        assert_eq!(result.get("action").unwrap(), "Swipe");
    }

    #[test]
    fn test_action_result_success() {
        let result = ActionResult::success();
        assert!(result.success);
        assert!(!result.should_finish);
    }

    #[test]
    fn test_action_result_finish() {
        let result = ActionResult::finish(Some("Done".to_string()));
        assert!(result.success);
        assert!(result.should_finish);
        assert_eq!(result.message, Some("Done".to_string()));
    }
}

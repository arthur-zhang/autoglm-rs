/// Internationalization (i18n) module for Phone Agent UI messages
use phf::phf_map;
use serde::{Deserialize, Serialize};

/// Language options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    Chinese,
    English,
}

impl Language {
    /// Parse language from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "en" | "english" => Self::English,
            _ => Self::Chinese,
        }
    }

    /// Get language code string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chinese => "cn",
            Self::English => "en",
        }
    }
}

/// Chinese messages
pub static MESSAGES_ZH: phf::Map<&'static str, &'static str> = phf_map! {
    "thinking" => "思考过程",
    "action" => "执行动作",
    "task_completed" => "任务完成",
    "done" => "完成",
    "starting_task" => "开始执行任务",
    "final_result" => "最终结果",
    "task_result" => "任务结果",
    "confirmation_required" => "需要确认",
    "continue_prompt" => "是否继续？(y/n)",
    "manual_operation_required" => "需要人工操作",
    "manual_operation_hint" => "请手动完成操作...",
    "press_enter_when_done" => "完成后按回车继续",
    "connection_failed" => "连接失败",
    "connection_successful" => "连接成功",
    "step" => "步骤",
    "task" => "任务",
    "result" => "结果",
    "performance_metrics" => "性能指标",
    "time_to_first_token" => "首 Token 延迟 (TTFT)",
    "time_to_thinking_end" => "思考完成延迟",
    "total_inference_time" => "总推理时间",
};

/// English messages
pub static MESSAGES_EN: phf::Map<&'static str, &'static str> = phf_map! {
    "thinking" => "Thinking",
    "action" => "Action",
    "task_completed" => "Task Completed",
    "done" => "Done",
    "starting_task" => "Starting task",
    "final_result" => "Final Result",
    "task_result" => "Task Result",
    "confirmation_required" => "Confirmation Required",
    "continue_prompt" => "Continue? (y/n)",
    "manual_operation_required" => "Manual Operation Required",
    "manual_operation_hint" => "Please complete the operation manually...",
    "press_enter_when_done" => "Press Enter when done",
    "connection_failed" => "Connection Failed",
    "connection_successful" => "Connection Successful",
    "step" => "Step",
    "task" => "Task",
    "result" => "Result",
    "performance_metrics" => "Performance Metrics",
    "time_to_first_token" => "Time to First Token (TTFT)",
    "time_to_thinking_end" => "Time to Thinking End",
    "total_inference_time" => "Total Inference Time",
};

/// Get UI messages dictionary by language
pub fn get_messages(lang: Language) -> &'static phf::Map<&'static str, &'static str> {
    match lang {
        Language::English => &MESSAGES_EN,
        Language::Chinese => &MESSAGES_ZH,
    }
}

/// Get a single UI message by key and language
/// Returns the message if found, otherwise returns the key as a fallback
pub fn get_message<'a>(key: &'a str, lang: Language) -> &'a str {
    let messages = get_messages(lang);
    match messages.get(key) {
        Some(msg) => msg,
        None => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_str("en"), Language::English);
        assert_eq!(Language::from_str("English"), Language::English);
        assert_eq!(Language::from_str("cn"), Language::Chinese);
        assert_eq!(Language::from_str("zh"), Language::Chinese);
    }

    #[test]
    fn test_get_message() {
        assert_eq!(get_message("thinking", Language::Chinese), "思考过程");
        assert_eq!(get_message("thinking", Language::English), "Thinking");
    }
}

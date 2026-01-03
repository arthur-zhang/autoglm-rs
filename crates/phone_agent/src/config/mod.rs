//! Configuration module for phone_agent
//!
//! This module contains:
//! - `apps`: App package mappings for Android
//! - `timing`: Timing configurations for device operations
//! - `i18n`: Internationalization support
//! - `prompts`: System prompts for AI agent

mod apps;
mod i18n;
mod prompts;
mod timing;

pub use apps::{get_app_name, get_package_name, list_supported_apps, APP_PACKAGES};
pub use i18n::{get_message, get_messages, Language, MESSAGES_EN, MESSAGES_ZH};
pub use prompts::get_system_prompt;
pub use timing::{
    ActionTimingConfig, ConnectionTimingConfig, DeviceTimingConfig, TimingConfig, TIMING_CONFIG,
};

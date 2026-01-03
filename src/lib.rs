/// AutoGLM-RS: Rust port of phone_agent
///
/// This library provides Android phone automation capabilities:
/// - ADB (Android Debug Bridge) utilities for device control
/// - AI-powered agent for visual understanding and task execution
/// - Action handling for model outputs
/// - Multi-language support (Chinese/English)

// Core ADB modules
pub mod error;
pub mod config;
pub mod connection;
pub mod device;
pub mod input;
pub mod screenshot;

// New modules ported from Python
pub mod i18n;
pub mod prompts;
pub mod device_factory;
pub mod model;
pub mod actions;
pub mod agent;

// Re-export commonly used types and functions
pub use error::{AdbError, Result};

pub use config::{
    ActionTimingConfig, ConnectionTimingConfig, DeviceTimingConfig, TimingConfig, TIMING_CONFIG,
    get_app_name, get_package_name, list_supported_apps, APP_PACKAGES,
};

pub use connection::{
    AdbConnection, ConnectionType, DeviceInfo, list_devices, quick_connect,
};

pub use device::{
    back, double_tap, get_current_app, home, launch_app, long_press, swipe, tap,
};

pub use input::{
    clear_text, detect_and_set_adb_keyboard, restore_keyboard, type_text,
};

pub use screenshot::{Screenshot, get_screenshot};

// New module exports
pub use i18n::{Language, get_message, get_messages};
pub use prompts::get_system_prompt;
pub use device_factory::{DeviceFactory, DeviceType, get_device_factory, set_device_type};
pub use model::{ModelClient, ModelConfig, ModelResponse, MessageBuilder};
pub use actions::{ActionHandler, ActionResult, parse_action, do_action, finish_action};
pub use agent::{PhoneAgent, AgentConfig, StepResult};

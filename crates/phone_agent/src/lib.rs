//! phone_agent: Rust port of Open-AutoGLM phone_agent
//!
//! This library provides phone automation capabilities:
//! - ADB (Android Debug Bridge) utilities for Android device control
//! - XCTest (placeholder) for iOS device control
//! - HDC (placeholder) for HarmonyOS device control
//! - AI-powered agent for visual understanding and task execution
//! - Action handling for model outputs
//! - Multi-language support (Chinese/English)
//!
//! # Example
//!
//! ```no_run
//! use phone_agent::{PhoneAgent, AgentConfig, ModelConfig, Language};
//!
//! #[tokio::main]
//! async fn main() {
//!     let model_config = ModelConfig::new("http://localhost:8000/v1", "autoglm-phone-9b");
//!     let agent_config = AgentConfig::new()
//!         .with_max_steps(50)
//!         .with_lang(Language::Chinese);
//!
//!     let mut agent = PhoneAgent::new(
//!         Some(model_config),
//!         Some(agent_config),
//!         None,
//!         None,
//!     );
//!
//!     let result = agent.run("打开微信").await;
//!     println!("Result: {:?}", result);
//! }
//! ```

// Core modules
pub mod error;

// Configuration module
pub mod config;

// Device backends
pub mod adb;
pub mod hdc;
pub mod xctest;

// Core functionality
pub mod actions;
pub mod agent;
pub mod device_factory;
pub mod model;

// Re-export commonly used types and functions
pub use error::{AdbError, Result};

// Config re-exports
pub use config::{
    get_app_name, get_message, get_messages, get_package_name, get_system_prompt,
    list_supported_apps, ActionTimingConfig, ConnectionTimingConfig, DeviceTimingConfig, Language,
    TimingConfig, APP_PACKAGES, MESSAGES_EN, MESSAGES_ZH, TIMING_CONFIG,
};

// ADB re-exports
pub use adb::{
    back, clear_text, detect_and_set_adb_keyboard, double_tap, get_current_app, get_screenshot,
    home, launch_app, list_devices, long_press, quick_connect, restore_keyboard, swipe, tap,
    type_text, AdbConnection, ConnectionType, DeviceInfo, Screenshot,
};

// Device factory re-exports
pub use device_factory::{get_device_factory, set_device_type, DeviceFactory, DeviceType};

// Model re-exports
pub use model::{MessageBuilder, ModelClient, ModelConfig, ModelResponse};

// Actions re-exports
pub use actions::{
    do_action, finish_action, parse_action, ActionHandler, ActionResult, ConfirmationCallback,
    TakeoverCallback,
};

// Agent re-exports
pub use agent::{AgentConfig, PhoneAgent, StepResult};

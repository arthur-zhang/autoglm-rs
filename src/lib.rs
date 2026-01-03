/// AutoGLM-RS: Rust port of phone_agent/adb module
///
/// This library provides ADB (Android Debug Bridge) utilities for Android device automation,
/// including connection management, device control, text input, and screenshot capture.

pub mod error;
pub mod config;
pub mod connection;
pub mod device;
pub mod input;
pub mod screenshot;

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

//! ADB (Android Debug Bridge) module for Android device control
//!
//! This module provides:
//! - `connection`: ADB connection management
//! - `device`: Device control operations (tap, swipe, back, home, etc.)
//! - `input`: Text input handling
//! - `screenshot`: Screenshot capture

mod connection;
mod device;
mod input;
mod screenshot;

pub use connection::{list_devices, quick_connect, AdbConnection, ConnectionType, DeviceInfo};
pub use device::{back, double_tap, get_current_app, home, launch_app, long_press, swipe, tap};
pub use input::{clear_text, detect_and_set_adb_keyboard, restore_keyboard, type_text};
pub use screenshot::{get_screenshot, Screenshot};

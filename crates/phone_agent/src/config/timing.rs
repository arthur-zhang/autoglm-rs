//! Timing configuration for device operations

use lazy_static::lazy_static;
use std::env;

/// Action timing configuration for text input operations
#[derive(Debug, Clone)]
pub struct ActionTimingConfig {
    pub keyboard_switch_delay: f64,
    pub text_clear_delay: f64,
    pub text_input_delay: f64,
    pub keyboard_restore_delay: f64,
}

impl Default for ActionTimingConfig {
    fn default() -> Self {
        Self {
            keyboard_switch_delay: env::var("PHONE_AGENT_KEYBOARD_SWITCH_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            text_clear_delay: env::var("PHONE_AGENT_TEXT_CLEAR_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            text_input_delay: env::var("PHONE_AGENT_TEXT_INPUT_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            keyboard_restore_delay: env::var("PHONE_AGENT_KEYBOARD_RESTORE_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
        }
    }
}

/// Device timing configuration for device operations
#[derive(Debug, Clone)]
pub struct DeviceTimingConfig {
    pub default_tap_delay: f64,
    pub default_double_tap_delay: f64,
    pub double_tap_interval: f64,
    pub default_long_press_delay: f64,
    pub default_swipe_delay: f64,
    pub default_back_delay: f64,
    pub default_home_delay: f64,
    pub default_launch_delay: f64,
}

impl Default for DeviceTimingConfig {
    fn default() -> Self {
        Self {
            default_tap_delay: env::var("PHONE_AGENT_TAP_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            default_double_tap_delay: env::var("PHONE_AGENT_DOUBLE_TAP_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            double_tap_interval: env::var("PHONE_AGENT_DOUBLE_TAP_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.1),
            default_long_press_delay: env::var("PHONE_AGENT_LONG_PRESS_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            default_swipe_delay: env::var("PHONE_AGENT_SWIPE_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            default_back_delay: env::var("PHONE_AGENT_BACK_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            default_home_delay: env::var("PHONE_AGENT_HOME_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            default_launch_delay: env::var("PHONE_AGENT_LAUNCH_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
        }
    }
}

/// Connection timing configuration for ADB connection operations
#[derive(Debug, Clone)]
pub struct ConnectionTimingConfig {
    pub adb_restart_delay: f64,
    pub server_restart_delay: f64,
}

impl Default for ConnectionTimingConfig {
    fn default() -> Self {
        Self {
            adb_restart_delay: env::var("PHONE_AGENT_ADB_RESTART_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2.0),
            server_restart_delay: env::var("PHONE_AGENT_SERVER_RESTART_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
        }
    }
}

/// Master timing configuration
#[derive(Debug, Clone)]
pub struct TimingConfig {
    pub action: ActionTimingConfig,
    pub device: DeviceTimingConfig,
    pub connection: ConnectionTimingConfig,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            action: ActionTimingConfig::default(),
            device: DeviceTimingConfig::default(),
            connection: ConnectionTimingConfig::default(),
        }
    }
}

lazy_static! {
    /// Global timing configuration instance
    pub static ref TIMING_CONFIG: TimingConfig = TimingConfig::default();
}

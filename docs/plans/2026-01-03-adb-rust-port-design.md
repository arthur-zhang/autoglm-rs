# ADB Rust Port Design

## Overview

Port the Python `phone_agent/adb` module to Rust, maintaining identical logic and behavior. The implementation will be a single library crate with modules for connection management, device control, input handling, screenshot capture, and configuration.

## Architecture

### Module Structure

```
src/
├── lib.rs           # Public API exports
├── error.rs         # Error types
├── connection.rs    # ADB connection management
├── device.rs        # Device control operations
├── input.rs         # Text input via ADB Keyboard
├── screenshot.rs    # Screenshot capture
└── config.rs        # Timing & app package configs
```

### Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["process", "time", "rt"] }
thiserror = "1"
anyhow = "1"
image = "0.25"
base64 = "0.22"
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }
tempfile = "3"
phf = { version = "0.11", features = ["macros"] }
lazy_static = "1.4"
```

## Error Handling

Custom error type using `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AdbError {
    #[error("Command execution failed: {0}")]
    CommandFailed(String),
    #[error("Connection timeout: {0}")]
    Timeout(String),
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}
```

Return types:
- Python `tuple[bool, str]` → Rust `Result<String, AdbError>`
- Python `str | None` → Rust `Result<Option<String>, AdbError>`

## Module Details

### 1. Connection Module (connection.rs)

**Types:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    Wifi,
    Remote,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub status: String,
    pub connection_type: ConnectionType,
    pub model: Option<String>,
    pub android_version: Option<String>,
}

pub struct AdbConnection {
    adb_path: String,
}
```

**Key Methods:**
- `connect(address: &str, timeout: u64) -> Result<String, AdbError>`
  - Auto-append `:5555` if port not specified
  - Check output for "connected" or "already connected"
  - Return error on timeout or failure

- `disconnect(address: Option<&str>) -> Result<String, AdbError>`
  - Disconnect specific device or all if None

- `list_devices() -> Result<Vec<DeviceInfo>, AdbError>`
  - Parse `adb devices -l` output
  - Extract device_id, status, model
  - Detect connection type (`:` in ID = Remote, else USB)

- `enable_tcpip(port: u16, device_id: Option<&str>) -> Result<String, AdbError>`
  - Enable TCP/IP mode on USB device
  - Wait for restart delay from config

- `get_device_ip(device_id: Option<&str>) -> Result<Option<String>, AdbError>`
  - Parse `ip route` output for src IP
  - Fallback to `ip addr show wlan0`

- `restart_server() -> Result<String, AdbError>`
  - Kill then start ADB server with delay

### 2. Device Module (device.rs)

All functions take `device_id: Option<&str>` and build command prefix accordingly.

**Key Functions:**
- `tap(x: i32, y: i32, device_id: Option<&str>, delay: Option<f64>) -> Result<(), AdbError>`
  - Execute `adb shell input tap x y`
  - Sleep for delay (from config if None)

- `double_tap(x: i32, y: i32, device_id: Option<&str>, delay: Option<f64>) -> Result<(), AdbError>`
  - Two taps with `double_tap_interval` between them
  - Sleep for delay after

- `long_press(x: i32, y: i32, duration_ms: u32, device_id: Option<&str>, delay: Option<f64>) -> Result<(), AdbError>`
  - Execute `adb shell input swipe x y x y duration_ms`

- `swipe(start_x: i32, start_y: i32, end_x: i32, end_y: i32, duration_ms: Option<u32>, device_id: Option<&str>, delay: Option<f64>) -> Result<(), AdbError>`
  - Auto-calculate duration if None: `dist_sq / 1000`, clamped to 1000-2000ms
  - Execute `adb shell input swipe start_x start_y end_x end_y duration_ms`

- `back(device_id: Option<&str>, delay: Option<f64>) -> Result<(), AdbError>`
  - Execute `adb shell input keyevent 4`

- `home(device_id: Option<&str>, delay: Option<f64>) -> Result<(), AdbError>`
  - Execute `adb shell input keyevent KEYCODE_HOME`

- `launch_app(app_name: &str, device_id: Option<&str>, delay: Option<f64>) -> Result<bool, AdbError>`
  - Lookup package in APP_PACKAGES
  - Execute `adb shell monkey -p package -c android.intent.category.LAUNCHER 1`
  - Return false if app not found

- `get_current_app(device_id: Option<&str>) -> Result<String, AdbError>`
  - Execute `adb shell dumpsys window`
  - Parse lines for "mCurrentFocus" or "mFocusedApp"
  - Match package against APP_PACKAGES
  - Return "System Home" if no match

### 3. Input Module (input.rs)

**Key Functions:**
- `type_text(text: &str, device_id: Option<&str>) -> Result<(), AdbError>`
  - Base64 encode text
  - Execute `adb shell am broadcast -a ADB_INPUT_B64 --es msg <encoded>`

- `clear_text(device_id: Option<&str>) -> Result<(), AdbError>`
  - Execute `adb shell am broadcast -a ADB_CLEAR_TEXT`

- `detect_and_set_adb_keyboard(device_id: Option<&str>) -> Result<String, AdbError>`
  - Get current IME: `adb shell settings get secure default_input_method`
  - If not ADB Keyboard, switch: `adb shell ime set com.android.adbkeyboard/.AdbIME`
  - Warm up with empty type_text call
  - Return original IME

- `restore_keyboard(ime: &str, device_id: Option<&str>) -> Result<(), AdbError>`
  - Execute `adb shell ime set <ime>`

### 4. Screenshot Module (screenshot.rs)

**Types:**
```rust
#[derive(Debug, Clone)]
pub struct Screenshot {
    pub base64_data: String,
    pub width: u32,
    pub height: u32,
    pub is_sensitive: bool,
}
```

**Key Function:**
- `get_screenshot(device_id: Option<&str>, timeout: u64) -> Result<Screenshot, AdbError>`
  - Generate temp path with UUID
  - Execute `adb shell screencap -p /sdcard/tmp.png`
  - Check output for "Status: -1" or "Failed" → return black fallback with `is_sensitive=true`
  - Pull: `adb pull /sdcard/tmp.png <temp_path>`
  - Load with `image` crate, get dimensions
  - Encode to PNG in memory buffer
  - Base64 encode
  - Clean up temp file
  - On error: return black 1080x2400 image with `is_sensitive=false`

### 5. Config Module (config.rs)

**Types:**
```rust
#[derive(Debug, Clone)]
pub struct ActionTimingConfig {
    pub keyboard_switch_delay: f64,
    pub text_clear_delay: f64,
    pub text_input_delay: f64,
    pub keyboard_restore_delay: f64,
}

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

#[derive(Debug, Clone)]
pub struct ConnectionTimingConfig {
    pub adb_restart_delay: f64,
    pub server_restart_delay: f64,
}

#[derive(Debug, Clone)]
pub struct TimingConfig {
    pub action: ActionTimingConfig,
    pub device: DeviceTimingConfig,
    pub connection: ConnectionTimingConfig,
}
```

**Implementation:**
- Use `lazy_static` for global `TIMING_CONFIG`
- Load from environment variables with defaults
- Use `phf::Map` for APP_PACKAGES (compile-time perfect hash map)
- Helper functions: `get_package_name()`, `get_app_name()`, `list_supported_apps()`

### 6. Public API (lib.rs)

Re-export all public types and functions:
```rust
pub use connection::{AdbConnection, ConnectionType, DeviceInfo, list_devices, quick_connect};
pub use device::{tap, swipe, back, home, double_tap, long_press, launch_app, get_current_app};
pub use input::{type_text, clear_text, detect_and_set_adb_keyboard, restore_keyboard};
pub use screenshot::{Screenshot, get_screenshot};
pub use config::{TimingConfig, TIMING_CONFIG, get_package_name, get_app_name, list_supported_apps};
pub use error::AdbError;
```

## Implementation Strategy

1. Set up Cargo.toml with dependencies
2. Implement error types (error.rs)
3. Implement config module (config.rs) - needed by other modules
4. Implement connection module (connection.rs)
5. Implement device module (device.rs)
6. Implement input module (input.rs)
7. Implement screenshot module (screenshot.rs)
8. Create lib.rs with public exports
9. Test each module against Python behavior

## Key Differences from Python

1. **Async/Sync**: Using tokio for async process execution, but providing sync wrappers
2. **Error handling**: Result types instead of tuple returns
3. **Type safety**: Strong typing for all parameters and return values
4. **Performance**: Compile-time hash map for app packages
5. **Memory safety**: Automatic cleanup, no manual resource management

## Testing Strategy

For each module, verify:
- Command construction matches Python exactly
- Output parsing produces identical results
- Error cases handled the same way
- Timing delays match configured values
- Edge cases (empty strings, None values, etc.) behave identically

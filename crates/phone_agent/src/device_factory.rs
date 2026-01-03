//! Device factory for selecting device backend (currently ADB only)

use crate::adb;
use crate::error::Result;
use std::sync::OnceLock;
use tokio::sync::RwLock;

/// Type of device connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceType {
    #[default]
    Adb,
    // XCTest and HDC are not implemented in this version
}

/// Factory for device-specific implementations
///
/// Currently only supports ADB (Android) devices.
/// XCTest (iOS) and HDC (HarmonyOS) support are not included in this Rust port.
#[derive(Debug, Clone)]
pub struct DeviceFactory {
    device_type: DeviceType,
}

impl DeviceFactory {
    /// Create a new device factory
    pub fn new(device_type: DeviceType) -> Self {
        Self { device_type }
    }

    /// Get the device type
    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }

    /// Get screenshot from device
    pub async fn get_screenshot(
        &self,
        device_id: Option<&str>,
        timeout: u64,
    ) -> Result<adb::Screenshot> {
        match self.device_type {
            DeviceType::Adb => adb::get_screenshot(device_id, timeout).await,
        }
    }

    /// Get current app name
    pub async fn get_current_app(&self, device_id: Option<&str>) -> Result<String> {
        match self.device_type {
            DeviceType::Adb => adb::get_current_app(device_id).await,
        }
    }

    /// Tap at coordinates
    pub async fn tap(
        &self,
        x: i32,
        y: i32,
        device_id: Option<&str>,
        delay: Option<f64>,
    ) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::tap(x, y, device_id, delay).await,
        }
    }

    /// Double tap at coordinates
    pub async fn double_tap(
        &self,
        x: i32,
        y: i32,
        device_id: Option<&str>,
        delay: Option<f64>,
    ) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::double_tap(x, y, device_id, delay).await,
        }
    }

    /// Long press at coordinates
    pub async fn long_press(
        &self,
        x: i32,
        y: i32,
        duration_ms: u32,
        device_id: Option<&str>,
        delay: Option<f64>,
    ) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::long_press(x, y, duration_ms, device_id, delay).await,
        }
    }

    /// Swipe from start to end
    pub async fn swipe(
        &self,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        duration_ms: Option<u32>,
        device_id: Option<&str>,
        delay: Option<f64>,
    ) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => {
                adb::swipe(start_x, start_y, end_x, end_y, duration_ms, device_id, delay).await
            }
        }
    }

    /// Press back button
    pub async fn back(&self, device_id: Option<&str>, delay: Option<f64>) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::back(device_id, delay).await,
        }
    }

    /// Press home button
    pub async fn home(&self, device_id: Option<&str>, delay: Option<f64>) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::home(device_id, delay).await,
        }
    }

    /// Launch an app
    pub async fn launch_app(
        &self,
        app_name: &str,
        device_id: Option<&str>,
        delay: Option<f64>,
    ) -> Result<bool> {
        match self.device_type {
            DeviceType::Adb => adb::launch_app(app_name, device_id, delay).await,
        }
    }

    /// Type text
    pub async fn type_text(&self, text: &str, device_id: Option<&str>) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::type_text(text, device_id).await,
        }
    }

    /// Clear text
    pub async fn clear_text(&self, device_id: Option<&str>) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::clear_text(device_id).await,
        }
    }

    /// Detect and set ADB keyboard
    pub async fn detect_and_set_adb_keyboard(&self, device_id: Option<&str>) -> Result<String> {
        match self.device_type {
            DeviceType::Adb => adb::detect_and_set_adb_keyboard(device_id).await,
        }
    }

    /// Restore keyboard
    pub async fn restore_keyboard(&self, ime: &str, device_id: Option<&str>) -> Result<()> {
        match self.device_type {
            DeviceType::Adb => adb::restore_keyboard(ime, device_id).await,
        }
    }

    /// List connected devices
    pub async fn list_devices(&self) -> Result<Vec<adb::DeviceInfo>> {
        match self.device_type {
            DeviceType::Adb => adb::list_devices().await,
        }
    }
}

impl Default for DeviceFactory {
    fn default() -> Self {
        Self::new(DeviceType::Adb)
    }
}

/// Global device factory instance
static DEVICE_FACTORY: OnceLock<RwLock<DeviceFactory>> = OnceLock::new();

/// Set the global device type
pub async fn set_device_type(device_type: DeviceType) {
    let factory = DEVICE_FACTORY.get_or_init(|| RwLock::new(DeviceFactory::default()));
    let mut guard = factory.write().await;
    *guard = DeviceFactory::new(device_type);
}

/// Get the global device factory instance
pub fn get_device_factory() -> &'static RwLock<DeviceFactory> {
    DEVICE_FACTORY.get_or_init(|| RwLock::new(DeviceFactory::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_type_default() {
        let factory = DeviceFactory::default();
        assert_eq!(factory.device_type(), DeviceType::Adb);
    }
}

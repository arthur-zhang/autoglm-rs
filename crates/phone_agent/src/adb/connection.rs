//! ADB connection management for local and remote devices

use crate::config::TIMING_CONFIG;
use crate::error::{AdbError, Result};
use std::time::Duration;
use tokio::process::Command;

/// Type of ADB connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    Wifi,
    Remote,
}

/// Information about a connected device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub status: String,
    pub connection_type: ConnectionType,
    pub model: Option<String>,
    pub android_version: Option<String>,
}

/// Manages ADB connections to Android devices
pub struct AdbConnection {
    adb_path: String,
}

impl AdbConnection {
    /// Create a new ADB connection manager
    pub fn new() -> Self {
        Self {
            adb_path: "adb".to_string(),
        }
    }

    /// Create a new ADB connection manager with custom ADB path
    pub fn with_path(adb_path: String) -> Self {
        Self { adb_path }
    }

    /// Connect to a remote device via TCP/IP
    pub async fn connect(&self, address: &str, timeout: u64) -> Result<String> {
        // Validate and normalize address format
        let address = if address.contains(':') {
            address.to_string()
        } else {
            format!("{}:5555", address)
        };

        let output = tokio::time::timeout(
            Duration::from_secs(timeout),
            Command::new(&self.adb_path)
                .arg("connect")
                .arg(&address)
                .output(),
        )
        .await
        .map_err(|_| AdbError::Timeout(format!("Connection timeout after {}s", timeout)))?
        .map_err(AdbError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        let lower = combined.to_lowercase();
        if lower.contains("connected") {
            Ok(format!("Connected to {}", address))
        } else if lower.contains("already connected") {
            Ok(format!("Already connected to {}", address))
        } else {
            Err(AdbError::CommandFailed(combined.trim().to_string()))
        }
    }

    /// Disconnect from a remote device
    pub async fn disconnect(&self, address: Option<&str>) -> Result<String> {
        let mut cmd = Command::new(&self.adb_path);
        cmd.arg("disconnect");

        if let Some(addr) = address {
            cmd.arg(addr);
        }

        let output = tokio::time::timeout(Duration::from_secs(5), cmd.output())
            .await
            .map_err(|_| AdbError::Timeout("Disconnect timeout after 5s".to_string()))?
            .map_err(AdbError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        let result = combined.trim();
        Ok(if result.is_empty() {
            "Disconnected".to_string()
        } else {
            result.to_string()
        })
    }

    /// List all connected devices
    pub async fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        let output = tokio::time::timeout(
            Duration::from_secs(5),
            Command::new(&self.adb_path)
                .arg("devices")
                .arg("-l")
                .output(),
        )
        .await
        .map_err(|_| AdbError::Timeout("List devices timeout after 5s".to_string()))?
        .map_err(AdbError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in stdout.lines().skip(1) {
            // Skip header line
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let device_id = parts[0].to_string();
                let status = parts[1].to_string();

                // Determine connection type
                let connection_type = if device_id.contains(':') {
                    ConnectionType::Remote
                } else if device_id.contains("emulator") {
                    ConnectionType::Usb
                } else {
                    ConnectionType::Usb
                };

                // Parse additional info
                let mut model = None;
                for part in &parts[2..] {
                    if part.starts_with("model:") {
                        model = part.split(':').nth(1).map(|s| s.to_string());
                        break;
                    }
                }

                devices.push(DeviceInfo {
                    device_id,
                    status,
                    connection_type,
                    model,
                    android_version: None,
                });
            }
        }

        Ok(devices)
    }

    /// Get detailed information about a device
    pub async fn get_device_info(&self, device_id: Option<&str>) -> Result<Option<DeviceInfo>> {
        let devices = self.list_devices().await?;

        if devices.is_empty() {
            return Ok(None);
        }

        if let Some(id) = device_id {
            Ok(devices.into_iter().find(|d| d.device_id == id))
        } else {
            Ok(devices.into_iter().next())
        }
    }

    /// Check if a device is connected
    pub async fn is_connected(&self, device_id: Option<&str>) -> Result<bool> {
        let devices = self.list_devices().await?;

        if devices.is_empty() {
            return Ok(false);
        }

        if let Some(id) = device_id {
            Ok(devices
                .iter()
                .any(|d| d.device_id == id && d.status == "device"))
        } else {
            Ok(devices.iter().any(|d| d.status == "device"))
        }
    }

    /// Enable TCP/IP debugging on a USB-connected device
    pub async fn enable_tcpip(&self, port: u16, device_id: Option<&str>) -> Result<String> {
        let mut cmd = Command::new(&self.adb_path);

        if let Some(id) = device_id {
            cmd.arg("-s").arg(id);
        }

        cmd.arg("tcpip").arg(port.to_string());

        let output = tokio::time::timeout(Duration::from_secs(10), cmd.output())
            .await
            .map_err(|_| AdbError::Timeout("Enable TCP/IP timeout after 10s".to_string()))?
            .map_err(AdbError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        if combined.to_lowercase().contains("restarting") || output.status.success() {
            tokio::time::sleep(Duration::from_secs_f64(
                TIMING_CONFIG.connection.adb_restart_delay,
            ))
            .await;
            Ok(format!("TCP/IP mode enabled on port {}", port))
        } else {
            Err(AdbError::CommandFailed(combined.trim().to_string()))
        }
    }

    /// Get the IP address of a connected device
    pub async fn get_device_ip(&self, device_id: Option<&str>) -> Result<Option<String>> {
        let mut cmd = Command::new(&self.adb_path);

        if let Some(id) = device_id {
            cmd.arg("-s").arg(id);
        }

        cmd.arg("shell").arg("ip").arg("route");

        let output = tokio::time::timeout(Duration::from_secs(5), cmd.output())
            .await
            .map_err(|_| AdbError::Timeout("Get device IP timeout after 5s".to_string()))?
            .map_err(AdbError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse IP from route output
        for line in stdout.lines() {
            if line.contains("src") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "src" && i + 1 < parts.len() {
                        return Ok(Some(parts[i + 1].to_string()));
                    }
                }
            }
        }

        // Alternative: try wlan0 interface
        let mut cmd = Command::new(&self.adb_path);

        if let Some(id) = device_id {
            cmd.arg("-s").arg(id);
        }

        cmd.arg("shell").arg("ip").arg("addr").arg("show").arg("wlan0");

        let output = tokio::time::timeout(Duration::from_secs(5), cmd.output())
            .await
            .map_err(|_| AdbError::Timeout("Get device IP timeout after 5s".to_string()))?
            .map_err(AdbError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.contains("inet ") {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    let ip = parts[1].split('/').next().unwrap_or("");
                    if !ip.is_empty() {
                        return Ok(Some(ip.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Restart the ADB server
    pub async fn restart_server(&self) -> Result<String> {
        // Kill server
        tokio::time::timeout(
            Duration::from_secs(5),
            Command::new(&self.adb_path)
                .arg("kill-server")
                .output(),
        )
        .await
        .map_err(|_| AdbError::Timeout("Kill server timeout after 5s".to_string()))?
        .map_err(AdbError::Io)?;

        tokio::time::sleep(Duration::from_secs_f64(
            TIMING_CONFIG.connection.server_restart_delay,
        ))
        .await;

        // Start server
        tokio::time::timeout(
            Duration::from_secs(5),
            Command::new(&self.adb_path)
                .arg("start-server")
                .output(),
        )
        .await
        .map_err(|_| AdbError::Timeout("Start server timeout after 5s".to_string()))?
        .map_err(AdbError::Io)?;

        Ok("ADB server restarted".to_string())
    }
}

impl Default for AdbConnection {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick helper to connect to a remote device
pub async fn quick_connect(address: &str) -> Result<String> {
    let conn = AdbConnection::new();
    conn.connect(address, 10).await
}

/// Quick helper to list connected devices
pub async fn list_devices() -> Result<Vec<DeviceInfo>> {
    let conn = AdbConnection::new();
    conn.list_devices().await
}

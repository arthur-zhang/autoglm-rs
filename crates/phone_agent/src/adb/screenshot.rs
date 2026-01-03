//! Screenshot utilities for capturing Android device screen

use crate::error::{AdbError, Result};
use base64::{engine::general_purpose, Engine as _};
use image::{ImageBuffer, Rgb};
use std::io::Cursor;
use std::time::Duration;
use tempfile::tempdir;
use tokio::process::Command;
use tracing::{debug, warn};

/// Represents a captured screenshot
#[derive(Debug, Clone)]
pub struct Screenshot {
    pub base64_data: String,
    pub width: u32,
    pub height: u32,
    pub is_sensitive: bool,
}

/// Build ADB command prefix with optional device specifier
fn get_adb_prefix(device_id: Option<&str>) -> Vec<String> {
    let mut prefix = vec!["adb".to_string()];
    if let Some(id) = device_id {
        prefix.push("-s".to_string());
        prefix.push(id.to_string());
    }
    prefix
}

/// Create a black fallback image when screenshot fails
fn create_fallback_screenshot(is_sensitive: bool, reason: &str) -> Screenshot {
    warn!("Creating fallback screenshot: {}", reason);

    let default_width = 1080u32;
    let default_height = 2400u32;

    let black_img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(default_width, default_height, Rgb([0, 0, 0]));

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    black_img
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();

    let base64_data = general_purpose::STANDARD.encode(&buffer);

    Screenshot {
        base64_data,
        width: default_width,
        height: default_height,
        is_sensitive,
    }
}

/// Capture a screenshot from the connected Android device
pub async fn get_screenshot(device_id: Option<&str>, timeout: u64) -> Result<Screenshot> {
    // Use a temp directory so the file doesn't exist until adb pull creates it
    let temp_dir = tempdir().map_err(AdbError::Io)?;
    let temp_path = temp_dir.path().join("screenshot.png");
    let prefix = get_adb_prefix(device_id);

    debug!("Capturing screenshot with device_id: {:?}", device_id);

    // Execute screenshot command on device
    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("screencap")
        .arg("-p")
        .arg("/sdcard/tmp.png");

    let output = tokio::time::timeout(Duration::from_secs(timeout), cmd.output())
        .await
        .map_err(|_| AdbError::Timeout(format!("Screenshot timeout after {}s", timeout)))?
        .map_err(AdbError::Io)?;

    // Check for screenshot failure (sensitive screen)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    debug!("screencap output: {}", combined);

    if combined.contains("Status: -1") || combined.contains("Failed") {
        return Ok(create_fallback_screenshot(
            true,
            "screencap returned Status: -1 or Failed (sensitive screen)",
        ));
    }

    // Pull screenshot to local temp path
    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("pull").arg("/sdcard/tmp.png").arg(&temp_path);

    let pull_output = tokio::time::timeout(Duration::from_secs(5), cmd.output())
        .await
        .map_err(|_| AdbError::Timeout("Screenshot pull timeout after 5s".to_string()))?
        .map_err(AdbError::Io)?;

    // Check if adb pull succeeded
    let pull_stdout = String::from_utf8_lossy(&pull_output.stdout);
    let pull_stderr = String::from_utf8_lossy(&pull_output.stderr);
    let pull_combined = format!("{}{}", pull_stdout, pull_stderr);

    debug!("adb pull output: {}", pull_combined);

    // adb pull prints "pulled" on success, or error messages on failure
    if !pull_output.status.success() {
        return Ok(create_fallback_screenshot(
            false,
            &format!("adb pull failed: {}", pull_combined),
        ));
    }

    // Check if file exists and has content
    if !temp_path.exists() {
        return Ok(create_fallback_screenshot(
            false,
            "Screenshot file does not exist after adb pull",
        ));
    }

    let file_size = std::fs::metadata(&temp_path)
        .map(|m| m.len())
        .unwrap_or(0);

    if file_size == 0 {
        return Ok(create_fallback_screenshot(
            false,
            "Screenshot file is empty (0 bytes)",
        ));
    }

    debug!("Screenshot file size: {} bytes", file_size);

    // Read and encode image
    let img = match image::open(&temp_path) {
        Ok(img) => img,
        Err(e) => {
            return Ok(create_fallback_screenshot(
                false,
                &format!("Failed to decode image: {}", e),
            ));
        }
    };

    let width = img.width();
    let height = img.height();

    debug!("Screenshot dimensions: {}x{}", width, height);

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(AdbError::Image)?;

    let base64_data = general_purpose::STANDARD.encode(&buffer);

    // Cleanup is automatic when temp_dir goes out of scope

    Ok(Screenshot {
        base64_data,
        width,
        height,
        is_sensitive: false,
    })
}

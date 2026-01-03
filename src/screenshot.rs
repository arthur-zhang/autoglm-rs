/// Screenshot utilities for capturing Android device screen
use crate::error::{AdbError, Result};
use base64::{engine::general_purpose, Engine as _};
use image::{ImageBuffer, Rgb};
use std::io::Cursor;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::process::Command;

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
fn create_fallback_screenshot(is_sensitive: bool) -> Screenshot {
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
    let temp_file = NamedTempFile::new().map_err(|e| AdbError::Io(e))?;
    let temp_path = temp_file.path().to_path_buf();
    let prefix = get_adb_prefix(device_id);

    // Execute screenshot command
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
        .map_err(|e| AdbError::Io(e))?;

    // Check for screenshot failure (sensitive screen)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    if combined.contains("Status: -1") || combined.contains("Failed") {
        return Ok(create_fallback_screenshot(true));
    }

    // Pull screenshot to local temp path
    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("pull")
        .arg("/sdcard/tmp.png")
        .arg(&temp_path);

    tokio::time::timeout(Duration::from_secs(5), cmd.output())
        .await
        .map_err(|_| AdbError::Timeout("Screenshot pull timeout after 5s".to_string()))?
        .map_err(|e| AdbError::Io(e))?;

    // Check if file exists
    if !temp_path.exists() {
        return Ok(create_fallback_screenshot(false));
    }

    // Read and encode image
    let img = match image::open(&temp_path) {
        Ok(img) => img,
        Err(_) => return Ok(create_fallback_screenshot(false)),
    };

    let width = img.width();
    let height = img.height();

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| AdbError::Image(e))?;

    let base64_data = general_purpose::STANDARD.encode(&buffer);

    // Cleanup is automatic with NamedTempFile

    Ok(Screenshot {
        base64_data,
        width,
        height,
        is_sensitive: false,
    })
}

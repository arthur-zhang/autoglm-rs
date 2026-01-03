/// Input utilities for Android device text input
use crate::error::{AdbError, Result};
use base64::{engine::general_purpose, Engine as _};
use tokio::process::Command;

/// Build ADB command prefix with optional device specifier
fn get_adb_prefix(device_id: Option<&str>) -> Vec<String> {
    let mut prefix = vec!["adb".to_string()];
    if let Some(id) = device_id {
        prefix.push("-s".to_string());
        prefix.push(id.to_string());
    }
    prefix
}

/// Type text into the currently focused input field using ADB Keyboard
pub async fn type_text(text: &str, device_id: Option<&str>) -> Result<()> {
    let prefix = get_adb_prefix(device_id);
    let encoded_text = general_purpose::STANDARD.encode(text.as_bytes());

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("am")
        .arg("broadcast")
        .arg("-a")
        .arg("ADB_INPUT_B64")
        .arg("--es")
        .arg("msg")
        .arg(&encoded_text);

    cmd.output().await.map_err(|e| AdbError::Io(e))?;

    Ok(())
}

/// Clear text in the currently focused input field
pub async fn clear_text(device_id: Option<&str>) -> Result<()> {
    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("am")
        .arg("broadcast")
        .arg("-a")
        .arg("ADB_CLEAR_TEXT");

    cmd.output().await.map_err(|e| AdbError::Io(e))?;

    Ok(())
}

/// Detect current keyboard and switch to ADB Keyboard if needed
pub async fn detect_and_set_adb_keyboard(device_id: Option<&str>) -> Result<String> {
    let prefix = get_adb_prefix(device_id);

    // Get current IME
    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("settings")
        .arg("get")
        .arg("secure")
        .arg("default_input_method");

    let output = cmd.output().await.map_err(|e| AdbError::Io(e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let current_ime = format!("{}{}", stdout, stderr).trim().to_string();

    // Switch to ADB Keyboard if not already set
    if !current_ime.contains("com.android.adbkeyboard/.AdbIME") {
        let mut cmd = Command::new(&prefix[0]);
        for arg in &prefix[1..] {
            cmd.arg(arg);
        }
        cmd.arg("shell")
            .arg("ime")
            .arg("set")
            .arg("com.android.adbkeyboard/.AdbIME");

        cmd.output().await.map_err(|e| AdbError::Io(e))?;
    }

    // Warm up the keyboard
    type_text("", device_id).await?;

    Ok(current_ime)
}

/// Restore the original keyboard IME
pub async fn restore_keyboard(ime: &str, device_id: Option<&str>) -> Result<()> {
    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell").arg("ime").arg("set").arg(ime);

    cmd.output().await.map_err(|e| AdbError::Io(e))?;

    Ok(())
}

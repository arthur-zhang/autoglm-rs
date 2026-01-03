//! Device control utilities for Android automation

use crate::config::{get_package_name, APP_PACKAGES, TIMING_CONFIG};
use crate::error::{AdbError, Result};
use std::time::Duration;
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

/// Get the currently focused app name
pub async fn get_current_app(device_id: Option<&str>) -> Result<String> {
    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell").arg("dumpsys").arg("window");

    let output = cmd.output().await.map_err(AdbError::Io)?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.is_empty() {
        return Err(AdbError::CommandFailed(
            "No output from dumpsys window".to_string(),
        ));
    }

    // Parse window focus info
    for line in stdout.lines() {
        if line.contains("mCurrentFocus") || line.contains("mFocusedApp") {
            for (app_name, package) in APP_PACKAGES.entries() {
                if line.contains(package) {
                    return Ok(app_name.to_string());
                }
            }
        }
    }

    Ok("System Home".to_string())
}

/// Tap at the specified coordinates
pub async fn tap(x: i32, y: i32, device_id: Option<&str>, delay: Option<f64>) -> Result<()> {
    let delay = delay.unwrap_or(TIMING_CONFIG.device.default_tap_delay);
    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("input")
        .arg("tap")
        .arg(x.to_string())
        .arg(y.to_string());

    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(delay)).await;
    Ok(())
}

/// Double tap at the specified coordinates
pub async fn double_tap(
    x: i32,
    y: i32,
    device_id: Option<&str>,
    delay: Option<f64>,
) -> Result<()> {
    let delay = delay.unwrap_or(TIMING_CONFIG.device.default_double_tap_delay);
    let prefix = get_adb_prefix(device_id);

    // First tap
    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("input")
        .arg("tap")
        .arg(x.to_string())
        .arg(y.to_string());
    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(
        TIMING_CONFIG.device.double_tap_interval,
    ))
    .await;

    // Second tap
    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("input")
        .arg("tap")
        .arg(x.to_string())
        .arg(y.to_string());
    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(delay)).await;
    Ok(())
}

/// Long press at the specified coordinates
pub async fn long_press(
    x: i32,
    y: i32,
    duration_ms: u32,
    device_id: Option<&str>,
    delay: Option<f64>,
) -> Result<()> {
    let delay = delay.unwrap_or(TIMING_CONFIG.device.default_long_press_delay);
    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("input")
        .arg("swipe")
        .arg(x.to_string())
        .arg(y.to_string())
        .arg(x.to_string())
        .arg(y.to_string())
        .arg(duration_ms.to_string());

    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(delay)).await;
    Ok(())
}

/// Swipe from start to end coordinates
pub async fn swipe(
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    duration_ms: Option<u32>,
    device_id: Option<&str>,
    delay: Option<f64>,
) -> Result<()> {
    let delay = delay.unwrap_or(TIMING_CONFIG.device.default_swipe_delay);
    let prefix = get_adb_prefix(device_id);

    // Calculate duration based on distance if not provided
    let duration_ms = duration_ms.unwrap_or_else(|| {
        let dist_sq = ((start_x - end_x).pow(2) + (start_y - end_y).pow(2)) as u32;
        let duration = dist_sq / 1000;
        duration.clamp(1000, 2000)
    });

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("input")
        .arg("swipe")
        .arg(start_x.to_string())
        .arg(start_y.to_string())
        .arg(end_x.to_string())
        .arg(end_y.to_string())
        .arg(duration_ms.to_string());

    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(delay)).await;
    Ok(())
}

/// Press the back button
pub async fn back(device_id: Option<&str>, delay: Option<f64>) -> Result<()> {
    let delay = delay.unwrap_or(TIMING_CONFIG.device.default_back_delay);
    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell").arg("input").arg("keyevent").arg("4");

    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(delay)).await;
    Ok(())
}

/// Press the home button
pub async fn home(device_id: Option<&str>, delay: Option<f64>) -> Result<()> {
    let delay = delay.unwrap_or(TIMING_CONFIG.device.default_home_delay);
    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("input")
        .arg("keyevent")
        .arg("KEYCODE_HOME");

    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(delay)).await;
    Ok(())
}

/// Launch an app by name
pub async fn launch_app(
    app_name: &str,
    device_id: Option<&str>,
    delay: Option<f64>,
) -> Result<bool> {
    let delay = delay.unwrap_or(TIMING_CONFIG.device.default_launch_delay);

    let package = match get_package_name(app_name) {
        Some(pkg) => pkg,
        None => return Ok(false),
    };

    let prefix = get_adb_prefix(device_id);

    let mut cmd = Command::new(&prefix[0]);
    for arg in &prefix[1..] {
        cmd.arg(arg);
    }
    cmd.arg("shell")
        .arg("monkey")
        .arg("-p")
        .arg(package)
        .arg("-c")
        .arg("android.intent.category.LAUNCHER")
        .arg("1");

    cmd.output().await.map_err(AdbError::Io)?;

    tokio::time::sleep(Duration::from_secs_f64(delay)).await;
    Ok(true)
}

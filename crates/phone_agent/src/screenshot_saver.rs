//! Screenshot saving utilities for persisting screenshots to disk

use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

use crate::error::{AdbError, Result};

/// Manages screenshot persistence with timestamped directories and filenames
#[derive(Debug, Clone)]
pub struct ScreenshotSaver {
    /// Base directory for saving screenshots
    base_dir: PathBuf,
    /// Session directory (created at session start with timestamp)
    session_dir: PathBuf,
    /// Step counter for ordering screenshots
    step_count: usize,
}

impl ScreenshotSaver {
    /// Create a new ScreenshotSaver
    ///
    /// Creates a session subdirectory with format: `yyyy-mm-dd_HH-MM-SS-mmm`
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for saving screenshots
    ///
    /// # Returns
    /// A new ScreenshotSaver instance
    pub async fn new(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        let session_start: DateTime<Local> = Local::now();

        // Format: yyyy-mm-dd_HH-MM-SS-mmm
        let session_name = session_start.format("%Y-%m-%d_%H-%M-%S-%3f").to_string();
        let session_dir = base_dir.join(&session_name);

        // Create directories
        fs::create_dir_all(&session_dir)
            .await
            .map_err(AdbError::Io)?;

        info!("Screenshot session directory: {}", session_dir.display());

        Ok(Self {
            base_dir,
            session_dir,
            step_count: 0,
        })
    }

    /// Save a screenshot to the session directory
    ///
    /// Filename format: `step_NNN_yyyy-mm-dd_HH-MM-SS-mmm.png`
    ///
    /// # Arguments
    /// * `base64_data` - Base64-encoded PNG image data
    ///
    /// # Returns
    /// Path to the saved screenshot
    pub async fn save(&mut self, base64_data: &str) -> Result<PathBuf> {
        self.step_count += 1;
        let now: DateTime<Local> = Local::now();

        // Format: step_NNN_yyyy-mm-dd_HH-MM-SS-mmm.png
        let filename = format!(
            "step_{:03}_{}.png",
            self.step_count,
            now.format("%Y-%m-%d_%H-%M-%S-%3f")
        );
        let file_path = self.session_dir.join(&filename);

        // Decode base64 and write to file
        let image_data = general_purpose::STANDARD
            .decode(base64_data)
            .map_err(|e| AdbError::CommandFailed(format!("Failed to decode base64: {}", e)))?;

        fs::write(&file_path, &image_data)
            .await
            .map_err(AdbError::Io)?;

        debug!(
            "Saved screenshot: {} ({} bytes)",
            file_path.display(),
            image_data.len()
        );

        Ok(file_path)
    }

    /// Get the session directory path
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    /// Get the base directory path
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the current step count
    pub fn step_count(&self) -> usize {
        self.step_count
    }

    /// Reset step counter (for new task in interactive mode)
    pub fn reset_step_count(&mut self) {
        self.step_count = 0;
    }

    /// Create a new session directory (for new task in interactive mode)
    pub async fn new_session(&mut self) -> Result<()> {
        let session_start: DateTime<Local> = Local::now();
        let session_name = session_start.format("%Y-%m-%d_%H-%M-%S-%3f").to_string();
        self.session_dir = self.base_dir.join(&session_name);

        fs::create_dir_all(&self.session_dir)
            .await
            .map_err(AdbError::Io)?;

        self.step_count = 0;

        info!(
            "New screenshot session directory: {}",
            self.session_dir.display()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_screenshot_saver_creation() {
        let temp_dir = tempdir().unwrap();
        let saver = ScreenshotSaver::new(temp_dir.path()).await.unwrap();

        assert!(saver.session_dir().exists());
        assert_eq!(saver.step_count(), 0);
    }

    #[tokio::test]
    async fn test_screenshot_save() {
        let temp_dir = tempdir().unwrap();
        let mut saver = ScreenshotSaver::new(temp_dir.path()).await.unwrap();

        // Create a simple 1x1 PNG image and encode to base64
        let png_data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
            0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
            0x08, 0xD7, 0x63, 0xF8, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0xE7, 0x7C, 0xF4, 0xBE,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // IEND chunk
        ];
        let base64_data = general_purpose::STANDARD.encode(&png_data);

        let saved_path = saver.save(&base64_data).await.unwrap();

        assert!(saved_path.exists());
        assert_eq!(saver.step_count(), 1);
        assert!(saved_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("step_001_"));
    }
}

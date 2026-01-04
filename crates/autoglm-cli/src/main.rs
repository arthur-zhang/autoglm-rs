//! AutoGLM CLI - Command-line interface for phone automation
//!
//! Usage:
//!     autoglm [OPTIONS] [TASK]
//!
//! Environment Variables:
//!     PHONE_AGENT_BASE_URL: Model API base URL (default: http://localhost:8000/v1)
//!     PHONE_AGENT_MODEL: Model name (default: autoglm-phone-9b)
//!     PHONE_AGENT_API_KEY: API key for model authentication (default: EMPTY)
//!     PHONE_AGENT_MAX_STEPS: Maximum steps per task (default: 100)
//!     PHONE_AGENT_DEVICE_ID: ADB device ID for multi-device setups

use anyhow::{anyhow, Result};
use clap::Parser;
use phone_agent::{
    list_supported_apps, set_device_type, AdbConnection, AgentConfig, DeviceType, Language,
    ModelClient, ModelConfig, PhoneAgent,
};
use std::io::{self, BufRead, Write};
use std::time::Duration;
use tokio::process::Command;

/// Phone Agent - AI-powered phone automation
#[derive(Parser, Debug)]
#[command(name = "autoglm")]
#[command(about = "Phone Agent - AI-powered phone automation")]
#[command(after_help = r#"Examples:
    # Run with default settings (Android)
    autoglm

    # Specify model endpoint
    autoglm --base-url http://localhost:8000/v1

    # Use API key for authentication
    autoglm --apikey sk-xxxxx

    # Run with specific device
    autoglm --device-id emulator-5554

    # Connect to remote device
    autoglm --connect 192.168.1.100:5555

    # List connected devices
    autoglm --list-devices

    # Enable TCP/IP on USB device and get connection info
    autoglm --enable-tcpip

    # List supported apps
    autoglm --list-apps

    # Run a specific task
    autoglm "Open WeChat and send a message"
"#)]
struct Cli {
    // Model options
    /// Model API base URL
    #[arg(long, env = "PHONE_AGENT_BASE_URL", default_value = "http://localhost:8000/v1")]
    base_url: String,

    /// Model name
    #[arg(long, env = "PHONE_AGENT_MODEL", default_value = "autoglm-phone-9b")]
    model: String,

    /// API key for model authentication
    #[arg(long, env = "PHONE_AGENT_API_KEY", default_value = "EMPTY")]
    apikey: String,

    /// Maximum steps per task
    #[arg(long, env = "PHONE_AGENT_MAX_STEPS", default_value = "100")]
    max_steps: usize,

    // Device options
    /// ADB device ID
    #[arg(short = 'd', long, env = "PHONE_AGENT_DEVICE_ID")]
    device_id: Option<String>,

    /// Connect to remote device (e.g., 192.168.1.100:5555)
    #[arg(short = 'c', long, value_name = "ADDRESS")]
    connect: Option<String>,

    /// Disconnect from remote device (or 'all' to disconnect all)
    #[arg(long, value_name = "ADDRESS", num_args = 0..=1, default_missing_value = "all")]
    disconnect: Option<String>,

    /// List connected devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Enable TCP/IP debugging on USB device (default port: 5555)
    #[arg(long, value_name = "PORT", num_args = 0..=1, default_missing_value = "5555")]
    enable_tcpip: Option<u16>,

    // iOS specific options
    /// WebDriverAgent URL for iOS (default: http://localhost:8100)
    #[arg(long, env = "PHONE_AGENT_WDA_URL", default_value = "http://localhost:8100")]
    wda_url: String,

    /// Pair with iOS device (required for some operations)
    #[arg(long)]
    pair: bool,

    /// Show WebDriverAgent status and exit (iOS only)
    #[arg(long)]
    wda_status: bool,

    // Other options
    /// Suppress verbose output
    #[arg(short = 'q', long)]
    quiet: bool,

    /// List supported apps and exit
    #[arg(long)]
    list_apps: bool,

    /// Language for system prompt (cn or en, default: cn)
    #[arg(long, env = "PHONE_AGENT_LANG", default_value = "cn", value_parser = ["cn", "en"])]
    lang: String,

    /// Device type: adb for Android, hdc for HarmonyOS, ios for iPhone (default: adb)
    #[arg(long, env = "PHONE_AGENT_DEVICE_TYPE", default_value = "adb", value_parser = ["adb", "hdc", "ios"])]
    device_type: String,

    /// Directory to save screenshots (creates timestamped subdirectory per session)
    #[arg(long, env = "PHONE_AGENT_SCREENSHOT_DIR")]
    screenshot_dir: Option<String>,

    /// Task to execute (interactive mode if not provided)
    task: Option<String>,
}

/// Device type enum matching Python's DeviceType
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliDeviceType {
    Adb,
    Hdc,
    Ios,
}

impl CliDeviceType {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "adb" => Ok(Self::Adb),
            "hdc" => Ok(Self::Hdc),
            "ios" => Ok(Self::Ios),
            _ => Err(anyhow!("Invalid device type: {}", s)),
        }
    }

    fn tool_name(&self) -> &'static str {
        match self {
            Self::Adb => "ADB",
            Self::Hdc => "HDC",
            Self::Ios => "libimobiledevice",
        }
    }

    fn tool_cmd(&self) -> &'static str {
        match self {
            Self::Adb => "adb",
            Self::Hdc => "hdc",
            Self::Ios => "idevice_id",
        }
    }
}

/// Check system requirements before running the agent
async fn check_system_requirements(device_type: CliDeviceType, wda_url: &str) -> bool {
    println!("\u{1F50D} Checking system requirements...");
    println!("{}", "-".repeat(50));

    let mut all_passed = true;

    let tool_name = device_type.tool_name();
    let tool_cmd = device_type.tool_cmd();

    // Check 1: Tool installed
    print!("1. Checking {} installation... ", tool_name);
    io::stdout().flush().ok();

    if which::which(tool_cmd).is_err() {
        println!("\u{274C} FAILED");
        println!("   Error: {} is not installed or not in PATH.", tool_name);
        println!("   Solution: Install {}:", tool_name);
        match device_type {
            CliDeviceType::Adb => {
                println!("     - macOS: brew install android-platform-tools");
                println!("     - Linux: sudo apt install android-tools-adb");
                println!(
                    "     - Windows: Download from https://developer.android.com/studio/releases/platform-tools"
                );
            }
            CliDeviceType::Hdc => {
                println!(
                    "     - Download from HarmonyOS SDK or https://gitee.com/openharmony/docs"
                );
                println!("     - Add to PATH environment variable");
            }
            CliDeviceType::Ios => {
                println!("     - macOS: brew install libimobiledevice");
                println!("     - Linux: sudo apt-get install libimobiledevice-utils");
            }
        }
        all_passed = false;
    } else {
        // Double check by running version command
        let version_result = match device_type {
            CliDeviceType::Adb => {
                tokio::time::timeout(
                    Duration::from_secs(10),
                    Command::new(tool_cmd).arg("version").output(),
                )
                .await
            }
            CliDeviceType::Hdc => {
                tokio::time::timeout(
                    Duration::from_secs(10),
                    Command::new(tool_cmd).arg("-v").output(),
                )
                .await
            }
            CliDeviceType::Ios => {
                tokio::time::timeout(
                    Duration::from_secs(10),
                    Command::new(tool_cmd).arg("-l").output(),
                )
                .await
            }
        };

        match version_result {
            Ok(Ok(output)) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let version_line = stdout.lines().next().unwrap_or("installed");
                println!(
                    "\u{2705} OK ({})",
                    if version_line.is_empty() {
                        "installed"
                    } else {
                        version_line
                    }
                );
            }
            Ok(Ok(_)) => {
                println!("\u{274C} FAILED");
                println!("   Error: {} command failed to run.", tool_name);
                all_passed = false;
            }
            Ok(Err(_)) => {
                println!("\u{274C} FAILED");
                println!("   Error: {} command not found.", tool_name);
                all_passed = false;
            }
            Err(_) => {
                println!("\u{274C} FAILED");
                println!("   Error: {} command timed out.", tool_name);
                all_passed = false;
            }
        }
    }

    if !all_passed {
        println!("{}", "-".repeat(50));
        println!("\u{274C} System check failed. Please fix the issues above.");
        return false;
    }

    // Check 2: Device connected
    print!("2. Checking connected devices... ");
    io::stdout().flush().ok();

    let devices_result = match device_type {
        CliDeviceType::Adb => check_adb_devices().await,
        CliDeviceType::Hdc => check_hdc_devices().await,
        CliDeviceType::Ios => check_ios_devices().await,
    };

    match devices_result {
        Ok(devices) if devices.is_empty() => {
            println!("\u{274C} FAILED");
            println!("   Error: No devices connected.");
            println!("   Solution:");
            match device_type {
                CliDeviceType::Adb => {
                    println!("     1. Enable USB debugging on your Android device");
                    println!("     2. Connect via USB and authorize the connection");
                    println!("     3. Or connect remotely: autoglm --connect <ip>:<port>");
                }
                CliDeviceType::Hdc => {
                    println!("     1. Enable USB debugging on your HarmonyOS device");
                    println!("     2. Connect via USB and authorize the connection");
                    println!(
                        "     3. Or connect remotely: autoglm --device-type hdc --connect <ip>:<port>"
                    );
                }
                CliDeviceType::Ios => {
                    println!("     1. Connect your iOS device via USB");
                    println!("     2. Unlock device and tap 'Trust This Computer'");
                    println!("     3. Verify: idevice_id -l");
                    println!("     4. Or connect via WiFi using device IP");
                }
            }
            all_passed = false;
        }
        Ok(devices) => {
            let display: Vec<&str> = devices.iter().take(2).map(|s| s.as_str()).collect();
            let suffix = if devices.len() > 2 { "..." } else { "" };
            println!(
                "\u{2705} OK ({} device(s): {}{})",
                devices.len(),
                display.join(", "),
                suffix
            );
        }
        Err(e) => {
            println!("\u{274C} FAILED");
            println!("   Error: {}", e);
            all_passed = false;
        }
    }

    if !all_passed {
        println!("{}", "-".repeat(50));
        println!("\u{274C} System check failed. Please fix the issues above.");
        return false;
    }

    // Check 3: ADB Keyboard (for ADB) or WebDriverAgent (for iOS) or skip for HDC
    match device_type {
        CliDeviceType::Adb => {
            print!("3. Checking ADB Keyboard... ");
            io::stdout().flush().ok();

            match check_adb_keyboard().await {
                Ok(true) => println!("\u{2705} OK"),
                Ok(false) => {
                    println!("\u{274C} FAILED");
                    println!("   Error: ADB Keyboard is not installed on the device.");
                    println!("   Solution:");
                    println!("     1. Download ADB Keyboard APK from:");
                    println!(
                        "        https://github.com/senzhk/ADBKeyBoard/blob/master/ADBKeyboard.apk"
                    );
                    println!("     2. Install it on your device: adb install ADBKeyboard.apk");
                    println!(
                        "     3. Enable it in Settings > System > Languages & Input > Virtual Keyboard"
                    );
                    all_passed = false;
                }
                Err(e) => {
                    println!("\u{274C} FAILED");
                    println!("   Error: {}", e);
                    all_passed = false;
                }
            }
        }
        CliDeviceType::Hdc => {
            print!("3. Skipping keyboard check for HarmonyOS... ");
            io::stdout().flush().ok();
            println!("\u{2705} OK (using native input)");
        }
        CliDeviceType::Ios => {
            print!("3. Checking WebDriverAgent ({})... ", wda_url);
            io::stdout().flush().ok();

            match check_wda_status(wda_url).await {
                Ok(true) => println!("\u{2705} OK"),
                Ok(false) => {
                    println!("\u{274C} FAILED");
                    println!("   Error: WebDriverAgent is not running or not accessible.");
                    println!("   Solution:");
                    println!("     1. Run WebDriverAgent on your iOS device via Xcode");
                    println!("     2. For USB: Set up port forwarding: iproxy 8100 8100");
                    println!(
                        "     3. For WiFi: Use device IP, e.g., --wda-url http://192.168.1.100:8100"
                    );
                    println!("     4. Verify in browser: open http://localhost:8100/status");
                    all_passed = false;
                }
                Err(e) => {
                    println!("\u{274C} FAILED");
                    println!("   Error: {}", e);
                    all_passed = false;
                }
            }
        }
    }

    println!("{}", "-".repeat(50));

    if all_passed {
        println!("\u{2705} All system checks passed!\n");
    } else {
        println!("\u{274C} System check failed. Please fix the issues above.");
    }

    all_passed
}

/// Check ADB devices
async fn check_adb_devices() -> Result<Vec<String>> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("adb").arg("devices").output(),
    )
    .await
    .map_err(|_| anyhow!("adb devices timeout"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices: Vec<String> = stdout
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty() && line.contains("\tdevice"))
        .map(|line| line.split('\t').next().unwrap_or("").to_string())
        .collect();

    Ok(devices)
}

/// Check HDC devices
async fn check_hdc_devices() -> Result<Vec<String>> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("hdc").arg("list").arg("targets").output(),
    )
    .await
    .map_err(|_| anyhow!("hdc list targets timeout"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices: Vec<String> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    Ok(devices)
}

/// Check iOS devices
async fn check_ios_devices() -> Result<Vec<String>> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("idevice_id").arg("-l").output(),
    )
    .await
    .map_err(|_| anyhow!("idevice_id -l timeout"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices: Vec<String> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    Ok(devices)
}

/// Check if ADB Keyboard is installed
async fn check_adb_keyboard() -> Result<bool> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("adb")
            .arg("shell")
            .arg("ime")
            .arg("list")
            .arg("-s")
            .output(),
    )
    .await
    .map_err(|_| anyhow!("adb shell ime list timeout"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("com.android.adbkeyboard/.AdbIME"))
}

/// Check WebDriverAgent status
async fn check_wda_status(wda_url: &str) -> Result<bool> {
    // Simple HTTP check - in production would use reqwest
    let status_url = format!("{}/status", wda_url.trim_end_matches('/'));

    // Use curl for simplicity
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("curl")
            .arg("-s")
            .arg("-o")
            .arg("/dev/null")
            .arg("-w")
            .arg("%{http_code}")
            .arg(&status_url)
            .output(),
    )
    .await
    .map_err(|_| anyhow!("WDA status check timeout"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim() == "200")
}

/// Check if the model API is accessible
async fn check_model_api(base_url: &str, model_name: &str, api_key: &str) -> bool {
    println!("\u{1F50D} Checking model API...");
    println!("{}", "-".repeat(50));

    print!("1. Checking API connectivity ({})... ", base_url);
    io::stdout().flush().ok();

    // Create model client and send a test request
    let model_config = ModelConfig::new(base_url, model_name).with_api_key(api_key);
    let client = ModelClient::new(model_config);

    match client.test_connection().await {
        Ok(_) => {
            println!("\u{2705} OK");
            println!("{}", "-".repeat(50));
            println!("\u{2705} Model API checks passed!\n");
            true
        }
        Err(e) => {
            println!("\u{274C} FAILED");
            let error_msg = e.to_string();

            if error_msg.contains("Connection refused") || error_msg.contains("Connection error") {
                println!("   Error: Cannot connect to {}", base_url);
                println!("   Solution:");
                println!("     1. Check if the model server is running");
                println!("     2. Verify the base URL is correct");
                println!("     3. Try: curl {}/chat/completions", base_url);
            } else if error_msg.to_lowercase().contains("timeout") {
                println!("   Error: Connection to {} timed out", base_url);
                println!("   Solution:");
                println!("     1. Check your network connection");
                println!("     2. Verify the server is responding");
            } else {
                println!("   Error: {}", error_msg);
            }

            println!("{}", "-".repeat(50));
            println!("\u{274C} Model API check failed. Please fix the issues above.");
            false
        }
    }
}

/// Handle device-related commands
async fn handle_device_commands(args: &Cli) -> Result<bool> {
    let device_type = CliDeviceType::from_str(&args.device_type)?;

    // Handle iOS-specific commands
    if device_type == CliDeviceType::Ios {
        return handle_ios_device_commands(args).await;
    }

    // Handle HDC-specific commands
    if device_type == CliDeviceType::Hdc {
        return handle_hdc_device_commands(args).await;
    }

    let conn = AdbConnection::new();

    // Handle --list-devices
    if args.list_devices {
        let devices = conn.list_devices().await?;
        if devices.is_empty() {
            println!("No devices connected.");
        } else {
            println!("Connected devices:");
            println!("{}", "-".repeat(60));
            for device in devices {
                let status_icon = if device.status == "device" {
                    "\u{2713}"
                } else {
                    "\u{2717}"
                };
                let conn_type = format!("{:?}", device.connection_type);
                let model_info = device
                    .model
                    .map(|m| format!(" ({})", m))
                    .unwrap_or_default();
                println!(
                    "  {} {:<30} [{}]{}",
                    status_icon, device.device_id, conn_type, model_info
                );
            }
        }
        return Ok(true);
    }

    // Handle --connect
    if let Some(addr) = &args.connect {
        println!("Connecting to {}...", addr);
        match conn.connect(addr, 10).await {
            Ok(msg) => {
                println!("\u{2713} {}", msg);
                return Ok(false); // Continue if connection succeeded
            }
            Err(e) => {
                println!("\u{2717} {}", e);
                return Ok(true);
            }
        }
    }

    // Handle --disconnect
    if let Some(addr) = &args.disconnect {
        if addr == "all" {
            println!("Disconnecting all remote devices...");
            match conn.disconnect(None).await {
                Ok(msg) => println!("\u{2713} {}", msg),
                Err(e) => println!("\u{2717} {}", e),
            }
        } else {
            println!("Disconnecting from {}...", addr);
            match conn.disconnect(Some(addr)).await {
                Ok(msg) => println!("\u{2713} {}", msg),
                Err(e) => println!("\u{2717} {}", e),
            }
        }
        return Ok(true);
    }

    // Handle --enable-tcpip
    if let Some(port) = args.enable_tcpip {
        println!("Enabling TCP/IP debugging on port {}...", port);

        match conn.enable_tcpip(port, args.device_id.as_deref()).await {
            Ok(msg) => {
                println!("\u{2713} {}", msg);

                // Try to get device IP
                if let Ok(Some(ip)) = conn.get_device_ip(args.device_id.as_deref()).await {
                    println!("\nYou can now connect remotely using:");
                    println!("  autoglm --connect {}:{}", ip, port);
                    println!("\nOr via ADB directly:");
                    println!("  adb connect {}:{}", ip, port);
                } else {
                    println!("\nCould not determine device IP. Check device WiFi settings.");
                }
            }
            Err(e) => println!("\u{2717} {}", e),
        }
        return Ok(true);
    }

    Ok(false)
}

/// Handle iOS device commands
async fn handle_ios_device_commands(args: &Cli) -> Result<bool> {
    // Handle --list-devices
    if args.list_devices {
        println!("iOS device listing is not yet implemented in this Rust version.");
        println!("Use: idevice_id -l");
        return Ok(true);
    }

    // Handle --pair
    if args.pair {
        println!("iOS device pairing is not yet implemented in this Rust version.");
        println!("Use: idevicepair pair");
        return Ok(true);
    }

    // Handle --wda-status
    if args.wda_status {
        println!("Checking WebDriverAgent status at {}...", args.wda_url);
        println!("{}", "-".repeat(50));

        match check_wda_status(&args.wda_url).await {
            Ok(true) => {
                println!("\u{2713} WebDriverAgent is running");
            }
            Ok(false) => {
                println!("\u{2717} WebDriverAgent is not running");
                println!("\nPlease start WebDriverAgent on your iOS device:");
                println!("  1. Open WebDriverAgent.xcodeproj in Xcode");
                println!("  2. Select your device");
                println!("  3. Run WebDriverAgentRunner (Product > Test or Cmd+U)");
                println!("  4. For USB: Run port forwarding: iproxy 8100 8100");
            }
            Err(e) => {
                println!("\u{2717} Error checking WDA status: {}", e);
            }
        }
        return Ok(true);
    }

    Ok(false)
}

/// Handle HDC device commands
async fn handle_hdc_device_commands(args: &Cli) -> Result<bool> {
    // Handle --list-devices
    if args.list_devices {
        println!("HDC device listing:");
        match check_hdc_devices().await {
            Ok(devices) if devices.is_empty() => {
                println!("No HarmonyOS devices connected.");
            }
            Ok(devices) => {
                println!("Connected HarmonyOS devices:");
                println!("{}", "-".repeat(60));
                for device in devices {
                    println!("  \u{2713} {}", device);
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
        return Ok(true);
    }

    // Handle --connect for HDC
    if let Some(addr) = &args.connect {
        println!("Connecting to HarmonyOS device at {}...", addr);
        let output = Command::new("hdc")
            .arg("tconn")
            .arg(addr)
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("{}{}", stdout, stderr);
        return Ok(false);
    }

    // Handle --disconnect for HDC
    if args.disconnect.is_some() {
        println!("HDC disconnect is not yet implemented in this Rust version.");
        return Ok(true);
    }

    Ok(false)
}

/// Print supported apps
fn print_supported_apps(device_type: CliDeviceType) {
    match device_type {
        CliDeviceType::Adb => {
            println!("Supported Android apps:");
            let mut apps: Vec<_> = list_supported_apps();
            apps.sort();
            for app in apps {
                println!("  - {}", app);
            }
        }
        CliDeviceType::Hdc => {
            println!("Supported HarmonyOS apps:");
            println!("  (HarmonyOS app list not yet implemented)");
        }
        CliDeviceType::Ios => {
            println!("Supported iOS apps:");
            println!("\nNote: For iOS apps, Bundle IDs are configured in:");
            println!("  phone_agent/config/apps_ios.py");
            println!("\n  (iOS app list not yet implemented)");
        }
    }
}

/// Print application header
fn print_header(args: &Cli, model_config: &ModelConfig, agent_config: &AgentConfig) {
    println!("{}", "=".repeat(50));
    match args.device_type.as_str() {
        "ios" => println!("Phone Agent iOS - AI-powered iOS automation"),
        _ => println!("Phone Agent - AI-powered phone automation"),
    }
    println!("{}", "=".repeat(50));
    println!("Model: {}", model_config.model_name);
    println!("Base URL: {}", model_config.base_url);
    println!("Max Steps: {}", agent_config.max_steps);
    println!("Language: {:?}", agent_config.lang);
    println!("Device Type: {}", args.device_type.to_uppercase());

    if args.device_type == "ios" {
        println!("WDA URL: {}", args.wda_url);
    }

    if let Some(device_id) = &args.device_id {
        println!("Device: {}", device_id);
    }

    if let Some(ref screenshot_dir) = agent_config.screenshot_dir {
        println!("Screenshot Dir: {}", screenshot_dir.display());
    }

    println!("{}", "=".repeat(50));
}

/// Run interactive mode
async fn run_interactive_mode(agent: &mut PhoneAgent) -> Result<()> {
    println!("\nEntering interactive mode. Type 'quit' to exit.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("Enter your task: ");
        stdout.flush()?;

        let mut input = String::new();
        match stdin.lock().read_line(&mut input) {
            Ok(0) => {
                // EOF
                println!("\nGoodbye!");
                break;
            }
            Ok(_) => {}
            Err(_) => {
                println!("\n\nInterrupted. Goodbye!");
                break;
            }
        }

        let task = input.trim();

        if task.eq_ignore_ascii_case("quit")
            || task.eq_ignore_ascii_case("exit")
            || task.eq_ignore_ascii_case("q")
        {
            println!("Goodbye!");
            break;
        }

        if task.is_empty() {
            continue;
        }

        println!();
        match agent.run(task).await {
            Ok(result) => println!("\nResult: {}\n", result),
            Err(e) => eprintln!("\nError: {}\n", e),
        }
        agent.reset().await;
    }

    Ok(())
}

/// Parse language string to Language enum
fn parse_lang(lang: &str) -> Language {
    match lang.to_lowercase().as_str() {
        "en" => Language::English,
        _ => Language::Chinese,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    // Parse device type
    let device_type = CliDeviceType::from_str(&args.device_type)?;

    // Set device type globally (for non-iOS)
    if device_type == CliDeviceType::Adb {
        set_device_type(DeviceType::Adb).await;
    }

    // Handle --list-apps (no system check needed)
    if args.list_apps {
        print_supported_apps(device_type);
        return Ok(());
    }

    // Handle device commands (may exit early)
    if handle_device_commands(&args).await? {
        return Ok(());
    }

    // Run system requirements check
    if !check_system_requirements(device_type, &args.wda_url).await {
        std::process::exit(1);
    }

    // Check model API
    if !check_model_api(&args.base_url, &args.model, &args.apikey).await {
        std::process::exit(1);
    }

    // Create configurations and agent
    let model_config = ModelConfig::new(&args.base_url, &args.model).with_api_key(&args.apikey);

    let lang = parse_lang(&args.lang);
    let mut agent_config = AgentConfig::new()
        .with_max_steps(args.max_steps)
        .with_lang(lang)
        .with_verbose(!args.quiet);

    if let Some(device_id) = &args.device_id {
        agent_config = agent_config.with_device_id(device_id);
    }

    if let Some(screenshot_dir) = &args.screenshot_dir {
        agent_config = agent_config.with_screenshot_dir(screenshot_dir);
    }

    // Print header
    print_header(&args, &model_config, &agent_config);

    // Create agent
    let mut agent = PhoneAgent::new(Some(model_config), Some(agent_config), None, None).await?;

    // Run with provided task or enter interactive mode
    if let Some(task) = &args.task {
        println!("\nTask: {}\n", task);
        let result = agent.run(task).await?;
        println!("\nResult: {}", result);
    } else {
        run_interactive_mode(&mut agent).await?;
    }

    Ok(())
}

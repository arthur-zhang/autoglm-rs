use autoglm_rs::*;

#[tokio::main]
async fn main() {
    println!("AutoGLM-RS ADB Library");
    println!("Example usage - listing connected devices:");

    match connection::list_devices().await {
        Ok(devices) => {
            if devices.is_empty() {
                println!("No devices connected");
            } else {
                for device in devices {
                    println!("  Device: {} ({})", device.device_id, device.status);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

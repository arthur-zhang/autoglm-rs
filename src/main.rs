use autoglm_rs::*;

#[tokio::main]
async fn main() {
    println!("AutoGLM-RS - Phone Agent Library");
    println!("=================================\n");

    // List connected devices
    println!("Connected devices:");
    match connection::list_devices().await {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  No devices connected");
            } else {
                for device in devices {
                    println!("  - {} ({})", device.device_id, device.status);
                }
            }
        }
        Err(e) => {
            eprintln!("  Error: {}", e);
        }
    }

    // Show available modules
    println!("\nAvailable modules:");
    println!("  - PhoneAgent: AI-powered phone automation agent");
    println!("  - ModelClient: OpenAI-compatible model client");
    println!("  - ActionHandler: Action execution handler");
    println!("  - DeviceFactory: Device abstraction layer");
    println!();

    // Example: Create agent configuration
    println!("Example agent configuration:");
    let agent_config = AgentConfig::new()
        .with_max_steps(50)
        .with_lang(Language::Chinese)
        .with_verbose(true);
    println!("  max_steps: {}", agent_config.max_steps);
    println!("  language: {:?}", agent_config.lang);
    println!("  verbose: {}", agent_config.verbose);

    // Example: Create model configuration
    println!("\nExample model configuration:");
    let model_config = ModelConfig::new("http://localhost:8000/v1", "autoglm-phone-9b");
    println!("  base_url: {}", model_config.base_url);
    println!("  model_name: {}", model_config.model_name);

    println!("\nTo use the PhoneAgent:");
    println!("  let mut agent = PhoneAgent::new(Some(model_config), Some(agent_config), None, None);");
    println!("  let result = agent.run(\"Open WeChat\").await;");
}

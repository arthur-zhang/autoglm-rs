//! Model client module for AI inference
//!
//! This module provides:
//! - `client`: OpenAI-compatible model client

mod client;

pub use client::{MessageBuilder, ModelClient, ModelConfig, ModelResponse};

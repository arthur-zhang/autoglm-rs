//! Action handling module for processing AI model outputs
//!
//! This module provides:
//! - `handler`: Action execution and processing

mod handler;

pub use handler::{
    do_action, finish_action, parse_action, ActionHandler, ActionResult, ConfirmationCallback,
    TakeoverCallback,
};

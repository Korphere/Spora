use serde::{Deserialize, Serialize};
use colored::*;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Logger;

impl Logger {
    pub fn log_step(action: &str, msg: &str) {
        println!("{:>12} {}", action.cyan().bold(), msg);
    }

    pub fn log_success(msg: &str)  {
        println!("{:>12} {}", "Success".green().bold(), msg);
    }

    pub fn log_info(action: &str, msg: &str) {
        println!("{:>12} {}", action.yellow().bold(), msg);
    }

    pub fn log_hint(msg: &str) {
        println!("{:>12} {}", "Hint".blue().bold(), msg);
    }

    pub fn log_error(msg: &str) {
        eprintln!("{:>12} {}", "Error".red().bold(), msg);
    }
}
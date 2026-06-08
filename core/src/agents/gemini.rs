//! Gemini agent adapter — uses the Gemini API.

use super::{Agent, find_binary};
use crate::{AgentStatus, GeminiConfig, HandoffResult};
use anyhow::Result;

pub struct GeminiAgent {
    api_key: Option<String>,
    model: String,
}

impl GeminiAgent {
    pub fn new(config: &GeminiConfig) -> Self {
        let api_key = config.api_key.clone()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .or_else(|| std::env::var("GOOGLE_API_KEY").ok());
        Self {
            api_key,
            model: config.model.clone(),
        }
    }
}

impl Agent for GeminiAgent {
    fn name(&self) -> &str { "gemini" }

    fn check_available(&self) -> AgentStatus {
        // First check if gemini CLI is available
        if find_binary("gemini").is_some() {
            return AgentStatus {
                name: "gemini".into(),
                available: true,
                reason: "Gemini CLI found in PATH".into(),
                version: None,
            };
        }

        match &self.api_key {
            Some(_) => AgentStatus {
                name: "gemini".into(),
                available: true,
                reason: format!("API key configured, model: {}", self.model),
                version: Some(self.model.clone()),
            },
            None => AgentStatus {
                name: "gemini".into(),
                available: false,
                reason: "No API key. Set GEMINI_API_KEY env var or add to config.toml".into(),
                version: None,
            },
        }
    }

    fn execute(&self, handoff_prompt: &str, project_dir: &str) -> Result<HandoffResult> {
        // Prefer the locally installed Gemini CLI — a live, interactive
        // handoff into the user's own gemini, not a one-shot API call.
        // `-i/--prompt-interactive` runs the handoff prompt and then stays
        // in the interactive TUI. stdio is inherited so the user lands in it.
        if let Some(binary) = find_binary("gemini") {
            let status = std::process::Command::new(&binary)
                .current_dir(project_dir)
                .arg("-i")
                .arg(handoff_prompt)
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()?;
            return Ok(HandoffResult {
                agent: "gemini".into(),
                success: status.success(),
                message: if status.success() {
                    "Gemini CLI session ended".into()
                } else {
                    format!("Gemini CLI exited with code {:?}", status.code())
                },
                handoff_file: None,
            });
        }

        // Fall back to API
        let api_key = self.api_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No Gemini API key"))?;

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, api_key
        );

        let body = serde_json::json!({
            "contents": [{
                "parts": [{ "text": handoff_prompt }]
            }]
        });

        let retry_config = crate::retry::RetryConfig::default();
        let url_clone = url.clone();
        let body_clone = body.clone();

        let resp = match crate::retry::with_retry(&retry_config, || {
            ureq::post(&url_clone)
                .set("Content-Type", "application/json")
                .send_json(&body_clone)
        }) {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, resp)) => {
                let error_body = resp.into_string().unwrap_or_default();
                let api_msg = serde_json::from_str::<serde_json::Value>(&error_body)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(String::from))
                    .unwrap_or(error_body);
                return Ok(HandoffResult {
                    agent: "gemini".into(),
                    success: false,
                    message: format!("Gemini API error (HTTP {}): {}", code, api_msg),
                    handoff_file: None,
                });
            }
            Err(ureq::Error::Transport(t)) => {
                return Ok(HandoffResult {
                    agent: "gemini".into(),
                    success: false,
                    message: format!("Gemini API unreachable: {}", t),
                    handoff_file: None,
                });
            }
        };

        let resp_json: serde_json::Value = resp.into_json()?;

        let text = resp_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("(no response)");

        println!("{text}");

        Ok(HandoffResult {
            agent: "gemini".into(),
            success: true,
            message: format!("Gemini ({}) responded to handoff", self.model),
            handoff_file: None,
        })
    }
}

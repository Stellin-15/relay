//! OpenCode agent adapter — Go-based coding agent.
//! https://github.com/opencode-ai/opencode

use super::{Agent, find_binary};
use crate::{AgentStatus, HandoffResult};
use anyhow::Result;
use std::process::Command;

pub struct OpenCodeAgent {
    binary: Option<String>,
}

impl OpenCodeAgent {
    pub fn new(config: &crate::OpenCodeConfig) -> Self {
        Self { binary: config.binary.clone() }
    }
}

impl Agent for OpenCodeAgent {
    fn name(&self) -> &str { "opencode" }

    fn check_available(&self) -> AgentStatus {
        match find_binary("opencode") {
            Some(path) => AgentStatus {
                name: "opencode".into(),
                available: true,
                reason: format!("Found at {path}"),
                version: Command::new(&path).arg("--version").output().ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()),
            },
            None => AgentStatus {
                name: "opencode".into(),
                available: false,
                reason: "Not found. Install: go install github.com/opencode-ai/opencode@latest".into(),
                version: None,
            },
        }
    }

    fn execute(&self, handoff_prompt: &str, project_dir: &str) -> Result<HandoffResult> {
        let binary = self.binary.clone()
            .or_else(|| find_binary("opencode"))
            .unwrap_or("opencode".into());
        let tmp = std::env::temp_dir().join("relay_handoff.md");
        std::fs::write(&tmp, handoff_prompt)?;

        // Open the interactive opencode TUI seeded with the handoff via the
        // TUI's `--prompt` flag, so the user keeps working in the session.
        // (A bare positional is read as a project path → ENAMETOOLONG;
        // `run` executes one-shot and exits — neither is a live handoff.)
        let status = Command::new(&binary)
            .current_dir(project_dir)
            .arg("--prompt")
            .arg(handoff_prompt)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()?;

        Ok(HandoffResult {
            agent: "opencode".into(),
            success: status.success(),
            message: "OpenCode session ended".into(),
            handoff_file: Some(tmp.to_string_lossy().to_string()),
        })
    }
}

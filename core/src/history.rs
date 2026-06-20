//! `relay history` — List all past handoffs with metadata.

use anyhow::Result;
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub struct HandoffEntry {
    pub filename: String,
    pub timestamp: String,
    pub agent: String,
    pub task: String,
}

/// List all handoff files in .relay/ directory.
pub fn list_handoffs(project_dir: &Path, limit: usize) -> Result<Vec<HandoffEntry>> {
    let relay_dir = project_dir.join(".relay");
    if !relay_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(String, std::time::SystemTime)> = Vec::new();
    for entry in std::fs::read_dir(&relay_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("handoff_") && name.ends_with(".md") {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    entries.push((entry.path().to_string_lossy().to_string(), modified));
                }
            }
        }
    }

    // Sort newest first
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(limit);

    let mut result = Vec::new();
    for (path, _) in entries {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let filename = Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let timestamp = parse_timestamp(&filename);
        let agent = extract_field(&content, "Target agent");
        let task = extract_field(&content, "## CURRENT TASK")
            .unwrap_or_else(|| extract_first_line_after(&content, "## CURRENT TASK"));

        result.push(HandoffEntry {
            filename,
            timestamp,
            agent: agent.unwrap_or_else(|| "unknown".into()),
            task: if task.chars().count() > 60 {
                format!("{}...", task.chars().take(57).collect::<String>())
            } else {
                task
            },
        });
    }

    Ok(result)
}

fn parse_timestamp(filename: &str) -> String {
    let ts = filename
        .strip_prefix("handoff_")
        .unwrap_or("")
        .strip_suffix(".md")
        .unwrap_or("");
    if ts.len() >= 15 {
        format!(
            "{}-{}-{} {}:{}:{}",
            &ts[0..4], &ts[4..6], &ts[6..8],
            &ts[9..11], &ts[11..13], &ts[13..15]
        )
    } else {
        ts.to_string()
    }
}

fn extract_field(content: &str, field: &str) -> Option<String> {
    for line in content.lines() {
        if line.contains(field) && line.contains(':') {
            let val = line.split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

fn extract_first_line_after(content: &str, header: &str) -> String {
    if let Some(pos) = content.find(header) {
        let after = &content[pos + header.len()..];
        for line in after.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("──") {
                return trimmed.to_string();
            }
        }
    }
    "unknown".into()
}

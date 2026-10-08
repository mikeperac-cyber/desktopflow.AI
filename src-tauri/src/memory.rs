//! Local operator memory (Phase 17).
//!
//! Memory is the operator's own long-lived context: preferences, corrections,
//! and per-application notes stored as JSON in the OS app configuration
//! directory (`memory.json`, next to `settings.json`). It never leaves the PC
//! except as a bounded, clearly labeled section of a planning prompt the user
//! explicitly requested — and hosted providers therefore receive memory text,
//! which the settings UI discloses with a purge control.
//!
//! Rules: at most 100 entries, content is short plain text, matches are exact
//! case-insensitive process names or global entries, and at most 5 entries
//! (2,000 chars) reach any single prompt. Memories are never executed, only
//! read by the planner alongside the fresh observation.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

const MEMORY_FILE_NAME: &str = "memory.json";

pub const MAX_MEMORY_ENTRIES: usize = 100;
const MAX_MEMORY_ID_CHARS: usize = 80;
pub const MAX_MEMORY_SUBJECT_CHARS: usize = 64;
pub const MAX_MEMORY_CONTENT_CHARS: usize = 500;
const MAX_PROMPT_ENTRIES: usize = 5;
const MAX_PROMPT_CHARS: usize = 2_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: String,
    pub subject: String,
    pub content: String,
    pub created_at_unix_ms: u64,
}

impl MemoryEntry {
    pub fn validate(&self, existing_ids: &[&str]) -> AppResult<()> {
        let id_length = self.id.trim().chars().count();
        if id_length == 0 || id_length > MAX_MEMORY_ID_CHARS {
            return Err(AppError::InvalidSettings(format!(
                "memory id must contain between 1 and {MAX_MEMORY_ID_CHARS} characters"
            )));
        }
        if existing_ids.contains(&self.id.as_str()) {
            return Err(AppError::InvalidSettings(format!(
                "memory id '{}' is duplicated",
                self.id
            )));
        }
        let subject_length = self.subject.trim().chars().count();
        if subject_length > MAX_MEMORY_SUBJECT_CHARS {
            return Err(AppError::InvalidSettings(format!(
                "memory subjects are limited to {MAX_MEMORY_SUBJECT_CHARS} characters"
            )));
        }
        let content_length = self.content.trim().chars().count();
        if content_length == 0 || content_length > MAX_MEMORY_CONTENT_CHARS {
            return Err(AppError::InvalidSettings(format!(
                "memory content must contain between 1 and {MAX_MEMORY_CONTENT_CHARS} characters"
            )));
        }
        Ok(())
    }

    /// An entry applies to a process when its subject names that process
    /// (case-insensitive) or when it has no subject at all.
    pub fn applies_to(&self, process_name: Option<&str>) -> bool {
        if self.subject.trim().is_empty() {
            return true;
        }
        process_name.is_some_and(|process| process.eq_ignore_ascii_case(self.subject.trim()))
    }
}

fn memory_path(app: &AppHandle) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join(MEMORY_FILE_NAME))
        .map_err(|_| AppError::SettingsStorage)
}

pub fn load_all(app: &AppHandle) -> AppResult<Vec<MemoryEntry>> {
    let path = memory_path(app)?;
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(AppError::SettingsStorage),
    };
    let mut entries: Vec<MemoryEntry> =
        serde_json::from_str(&contents).map_err(|_| AppError::SettingsStorage)?;
    entries.truncate(MAX_MEMORY_ENTRIES);
    Ok(entries)
}

fn persist_all(app: &AppHandle, entries: &[MemoryEntry]) -> AppResult<()> {
    let path = memory_path(app)?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(|_| AppError::SettingsStorage)?;
    }
    let payload = serde_json::to_vec_pretty(entries).map_err(|_| AppError::SettingsStorage)?;
    std::fs::write(path, payload).map_err(|_| AppError::SettingsStorage)
}

fn validate_all(entries: &[MemoryEntry]) -> AppResult<()> {
    if entries.len() > MAX_MEMORY_ENTRIES {
        return Err(AppError::InvalidSettings(format!(
            "at most {MAX_MEMORY_ENTRIES} memories are kept"
        )));
    }
    let mut seen: Vec<&str> = Vec::with_capacity(entries.len());
    for entry in entries {
        entry.validate(&seen)?;
        seen.push(entry.id.as_str());
    }
    Ok(())
}

pub fn add(app: &AppHandle, entry: MemoryEntry) -> AppResult<Vec<MemoryEntry>> {
    let mut entries = load_all(app)?;
    entries.push(entry);
    validate_all(&entries)?;
    persist_all(app, &entries)?;
    Ok(entries)
}

pub fn remove(app: &AppHandle, id: &str) -> AppResult<Vec<MemoryEntry>> {
    let mut entries = load_all(app)?;
    let before = entries.len();
    entries.retain(|entry| entry.id != id);
    if entries.len() == before {
        return Err(AppError::InvalidSettings(
            "no memory with that id exists".to_string(),
        ));
    }
    persist_all(app, &entries)?;
    Ok(entries)
}

pub fn purge(app: &AppHandle) -> AppResult<Vec<MemoryEntry>> {
    persist_all(app, &[])?;
    Ok(Vec::new())
}

/// Bounded, labeled operator context for one planning request. Empty when no
/// entry applies, so prompts without memory are byte-identical to before.
pub fn context_for_process(app: &AppHandle, process_name: Option<&str>) -> String {
    let entries = load_all(app).unwrap_or_default();
    let mut selected = entries
        .iter()
        .filter(|entry| entry.applies_to(process_name))
        .take(MAX_PROMPT_ENTRIES);
    let mut lines = Vec::new();
    let mut used = 0_usize;
    for entry in selected.by_ref() {
        let line = if entry.subject.trim().is_empty() {
            format!("- {}", entry.content.trim())
        } else {
            format!("- [{}] {}", entry.subject.trim(), entry.content.trim())
        };
        if used + line.chars().count() > MAX_PROMPT_CHARS {
            break;
        }
        used += line.chars().count();
        lines.push(line);
    }
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "Operator memory (user-supplied local notes, not verified facts):\n{}",
        lines.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, subject: &str, content: &str) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            subject: subject.to_string(),
            content: content.to_string(),
            created_at_unix_ms: 1_800_000_000_000,
        }
    }

    #[test]
    fn global_and_matching_entries_apply() {
        let global = entry("m1", "", "Prefer confirmation before closing windows.");
        let scoped = entry("m2", "notepad.exe", "Keep word wrap on.");
        assert!(global.applies_to(Some("EXPLORER.EXE")));
        assert!(global.applies_to(None));
        assert!(scoped.applies_to(Some("Notepad.exe")));
        assert!(!scoped.applies_to(Some("calc.exe")));
        assert!(!scoped.applies_to(None));
    }

    #[test]
    fn rejects_empty_content_and_duplicates() {
        let empty = entry("m1", "", "   ");
        assert!(empty.validate(&[]).is_err());
        let ok = entry("m1", "", "Something true.");
        assert!(ok.validate(&[]).is_ok());
        let duplicate = entry("m1", "", "Something else.");
        assert!(duplicate.validate(&["m1"]).is_err());
    }

    #[test]
    fn prompt_section_is_bounded_and_labeled() {
        let entries = vec![
            entry("m1", "", "First note."),
            entry("m2", "notepad.exe", "Second note."),
            entry("m3", "calc.exe", "Unrelated note."),
        ];
        // Simulate selection without touching the filesystem.
        let selected: Vec<&MemoryEntry> = entries
            .iter()
            .filter(|item| item.applies_to(Some("notepad.exe")))
            .take(MAX_PROMPT_ENTRIES)
            .collect();
        assert_eq!(selected.len(), 2);
        let rendered = format!(
            "Operator memory (user-supplied local notes, not verified facts):\n{}",
            selected
                .iter()
                .map(|item| format!("- {}", item.content))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(rendered.contains("Operator memory"));
        assert!(!rendered.contains("Unrelated"));
    }
}

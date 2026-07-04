use serde::Serialize;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub path: PathBuf,
    pub label: String,
    pub size_bytes: u64,
    /// Seconds since epoch, for JSON output / sorting; None if unavailable.
    pub last_modified_secs: Option<u64>,
    /// How to regenerate this artifact if it turns out you needed it.
    pub regenerate_hint: String,
}

impl Finding {
    pub fn age_days(&self) -> Option<u64> {
        let modified_secs = self.last_modified_secs?;
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        Some(now.saturating_sub(modified_secs) / 86_400)
    }
}

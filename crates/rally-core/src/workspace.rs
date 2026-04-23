use compact_str::CompactString;
use std::path::PathBuf;

use crate::ids::{Timestamp, WorkspaceId};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: CompactString,
    pub canonical_key: CompactString,
    pub repo: Option<PathBuf>,
    pub created_at: Timestamp,
    pub archived: bool,
}

impl Workspace {
    pub fn new(
        id: WorkspaceId,
        name: CompactString,
        repo: Option<PathBuf>,
        at: Timestamp,
    ) -> Self {
        let canonical_key = generate_canonical_key(&name, repo.as_deref(), at);
        Self {
            id,
            name,
            canonical_key,
            repo,
            created_at: at,
            archived: false,
        }
    }
}

/// Generate a canonical key for a workspace.
///
/// Format: `<sanitized-name>-<YYYYMMDD>T<HHmmss>`
/// The key is immutable once created and used as the durable storage key.
pub fn generate_canonical_key(
    name: &str,
    repo: Option<&std::path::Path>,
    at: Timestamp,
) -> CompactString {
    let sanitized = sanitize_for_key(name);

    // Derive a short repo component if available
    let repo_part = repo
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(sanitize_for_key)
        .unwrap_or_default();

    // Timestamp as compact sortable string (seconds since epoch, base36-ish)
    let ts = at.as_millis() / 1000;

    if repo_part.is_empty() {
        CompactString::from(format!("{sanitized}-{ts:010x}"))
    } else {
        CompactString::from(format!("{repo_part}-{sanitized}-{ts:010x}"))
    }
}

fn sanitize_for_key(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

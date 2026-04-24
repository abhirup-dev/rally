use compact_str::CompactString;
use std::path::PathBuf;
use tracing::debug;

use crate::ids::{Timestamp, WorkspaceId};

/// A project workspace grouping related agents.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Unique identifier.
    pub id: WorkspaceId,
    /// Human-readable name.
    pub name: CompactString,
    /// Immutable storage key derived from name + timestamp.
    pub canonical_key: CompactString,
    /// Optional path to the associated git repository.
    pub repo: Option<PathBuf>,
    /// When this workspace was created.
    pub created_at: Timestamp,
    /// Whether this workspace has been archived.
    pub archived: bool,
}

impl Workspace {
    /// Create a workspace, generating its canonical key from name + timestamp.
    pub fn new(id: WorkspaceId, name: CompactString, repo: Option<PathBuf>, at: Timestamp) -> Self {
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

    let key = if repo_part.is_empty() {
        CompactString::from(format!("{sanitized}-{ts:010x}"))
    } else {
        CompactString::from(format!("{repo_part}-{sanitized}-{ts:010x}"))
    };
    debug!(name, repo = ?repo, ts = at.as_millis(), %key, "canonical key generated");
    key
}

fn sanitize_for_key(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Html,
    Markdown,
    Mermaid,
    Diagram,
    #[default]
    Text,
}

impl fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactType::Html => write!(f, "html"),
            ArtifactType::Markdown => write!(f, "markdown"),
            ArtifactType::Mermaid => write!(f, "mermaid"),
            ArtifactType::Diagram => write!(f, "diagram"),
            ArtifactType::Text => write!(f, "text"),
        }
    }
}

impl ArtifactType {
    pub fn mime_type(&self) -> &str {
        match self {
            ArtifactType::Html => "text/html",
            ArtifactType::Markdown => "text/markdown",
            ArtifactType::Mermaid => "text/plain",
            ArtifactType::Diagram => "image/svg+xml",
            ArtifactType::Text => "text/plain",
        }
    }
}

impl From<&str> for ArtifactType {
    fn from(s: &str) -> Self {
        match s {
            "html" => ArtifactType::Html,
            "markdown" | "md" => ArtifactType::Markdown,
            "mermaid" => ArtifactType::Mermaid,
            "diagram" => ArtifactType::Diagram,
            _ => ArtifactType::Text,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub content: String,
    pub session_id: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitProvenance>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub session_id: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}

impl From<&Artifact> for ArtifactSummary {
    fn from(a: &Artifact) -> Self {
        ArtifactSummary {
            id: a.id.clone(),
            name: a.name.clone(),
            artifact_type: a.artifact_type.clone(),
            session_id: a.session_id.clone(),
            created_at: a.created_at,
            updated_at: a.updated_at,
            version: a.version,
            description: a.description.clone(),
            favicon: a.favicon.clone(),
            sender: a.sender.clone(),
            recipient: a.recipient.clone(),
            thread_id: a.thread_id.clone(),
            reply_to: a.reply_to.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PublishRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub content: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_id: Option<String>,
    /// Short human-readable name for this version, shown in history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// One or two emoji shown as the page icon and in the gallery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    /// Compare-and-swap guard: if set, the update only applies when the
    /// artifact's current version still equals this value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_version: Option<u32>,
    /// Handoff envelope: which agent/runtime published this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    /// Handoff envelope: which agent/runtime this is addressed to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// Handoff envelope: conversation thread this belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Handoff envelope: artifact id this replies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Git context of the publisher, captured best-effort by the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitProvenance>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishResponse {
    pub id: String,
    pub url: String,
    pub session_id: String,
    pub version: u32,
}

/// Best-effort git context captured where the publish happened.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub created_at: u64,
}

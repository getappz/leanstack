use crate::types::{
    Artifact, ArtifactSummary, ArtifactType, PublishRequest, PublishResponse, VersionInfo,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const ARTIFACTS_DIR: &str = "artifacts";
const META_FILE: &str = "meta.json";
const CONTENT_FILE: &str = "content";

const VERSIONS_DIR: &str = "versions";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ArtifactMeta {
    pub id: String,
    pub name: String,
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
    #[serde(default)]
    pub history: Vec<VersionInfo>,
}

#[derive(Clone)]
pub struct ArtifactStore {
    base_path: PathBuf,
    live_broadcast: Arc<Mutex<HashMap<String, Vec<std::sync::mpsc::Sender<String>>>>>,
}

impl ArtifactStore {
    pub fn new(base_path: PathBuf) -> Self {
        let store = ArtifactStore {
            base_path: base_path.join(ARTIFACTS_DIR),
            live_broadcast: Arc::new(Mutex::new(HashMap::new())),
        };
        let _ = fs::create_dir_all(&store.base_path);
        store
    }

    pub fn publish(&self, req: &PublishRequest) -> std::io::Result<PublishResponse> {
        let id = req
            .update_id
            .clone()
            .filter(|uid| self.artifact_dir(uid).exists())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let prev = self.read_meta(&id);

        if let (Some(base), Some(prev_meta)) = (req.base_version, prev.as_ref()) {
            if base != prev_meta.version {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "version conflict: base_version {base}, but current version is {} — re-read the artifact and retry",
                        prev_meta.version
                    ),
                ));
            }
        }

        let version = prev.as_ref().map(|m| m.version + 1).unwrap_or(1);
        let mut history = prev.as_ref().map(|m| m.history.clone()).unwrap_or_default();
        history.push(VersionInfo {
            version,
            label: req.label.clone(),
            created_at: now,
        });

        let meta = ArtifactMeta {
            id: id.clone(),
            name: req.name.clone(),
            artifact_type: req.artifact_type.clone(),
            session_id: req.session_id.clone(),
            created_at: prev.as_ref().map(|m| m.created_at).unwrap_or(now),
            updated_at: now,
            version,
            // Omitting description/favicon on an update keeps the old value.
            description: req
                .description
                .clone()
                .or_else(|| prev.as_ref().and_then(|m| m.description.clone())),
            favicon: req
                .favicon
                .clone()
                .or_else(|| prev.as_ref().and_then(|m| m.favicon.clone())),
            history,
        };

        let dir = self.artifact_dir(&id);
        fs::create_dir_all(dir.join(VERSIONS_DIR))?;
        fs::write(dir.join(VERSIONS_DIR).join(version.to_string()), &req.content)?;
        fs::write(dir.join(META_FILE), serde_json::to_string_pretty(&meta)?)?;
        fs::write(dir.join(CONTENT_FILE), &req.content)?;

        let url = format!("/{}", &id);
        let response = PublishResponse {
            id,
            url,
            session_id: req.session_id.clone(),
            version,
        };

        if prev.is_some() {
            self.broadcast(&response.id, "refresh");
        }

        Ok(response)
    }

    /// Version history, oldest first.
    pub fn versions(&self, id: &str) -> std::io::Result<Vec<VersionInfo>> {
        self.read_meta(id)
            .map(|m| m.history)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("artifact {id} not found"),
                )
            })
    }

    /// A specific version's snapshot; `get()` always serves the latest.
    pub fn get_version(&self, id: &str, version: u32) -> std::io::Result<Artifact> {
        let mut artifact = self.get(id)?;
        let content_path = self.artifact_dir(id).join(VERSIONS_DIR).join(version.to_string());
        artifact.content = fs::read_to_string(content_path)?;
        artifact.version = version;
        Ok(artifact)
    }

    pub fn get(&self, id: &str) -> std::io::Result<Artifact> {
        let dir = self.artifact_dir(id);
        if !dir.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("artifact {id} not found"),
            ));
        }
        let meta: ArtifactMeta =
            serde_json::from_str(&fs::read_to_string(dir.join(META_FILE))?)?;
        let content = fs::read_to_string(dir.join(CONTENT_FILE))?;
        Ok(Artifact {
            id: meta.id,
            name: meta.name,
            artifact_type: meta.artifact_type,
            content,
            session_id: meta.session_id,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            version: meta.version,
            description: meta.description,
            favicon: meta.favicon,
        })
    }

    pub fn list(&self, session_id: Option<&str>) -> std::io::Result<Vec<ArtifactSummary>> {
        let dir = &self.base_path;
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut artifacts = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let meta_path = entry.path().join(META_FILE);
            if meta_path.exists() {
                if let Some(meta) = fs::read_to_string(&meta_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<ArtifactMeta>(&s).ok())
                {
                    if session_id.is_none() || meta.session_id == session_id.unwrap() {
                        artifacts.push(ArtifactSummary {
                            id: meta.id,
                            name: meta.name,
                            artifact_type: meta.artifact_type,
                            session_id: meta.session_id,
                            created_at: meta.created_at,
                            updated_at: meta.updated_at,
                            version: meta.version,
                            description: meta.description,
                            favicon: meta.favicon,
                        });
                    }
                }
            }
        }
        artifacts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(artifacts)
    }

    pub fn delete(&self, id: &str) -> std::io::Result<bool> {
        let dir = self.artifact_dir(id);
        if !dir.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(dir)?;
        Ok(true)
    }

    pub fn subscribe(&self, id: &str) -> std::sync::mpsc::Receiver<String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut subs = self.live_broadcast.lock().unwrap();
        subs.entry(id.to_string()).or_default().push(tx);
        rx
    }

    fn broadcast(&self, id: &str, event: &str) {
        let mut subs = self.live_broadcast.lock().unwrap();
        if let Some(senders) = subs.remove(id) {
            for tx in senders {
                let _ = tx.send(event.to_string());
            }
        }
    }

    fn artifact_dir(&self, id: &str) -> PathBuf {
        self.base_path.join(id)
    }

    fn read_meta(&self, id: &str) -> Option<ArtifactMeta> {
        let path = self.artifact_dir(id).join(META_FILE);
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

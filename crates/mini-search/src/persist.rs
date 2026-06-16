use crate::document::Document;
use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::fields::FieldConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const CURRENT_VERSION: u32 = 1;
const STATE_FILE: &str = "state.json";

#[derive(Serialize, Deserialize)]
struct PersistedState {
    version: u32,
    documents: HashMap<String, HashMap<String, Document>>,
    field_configs: HashMap<String, HashMap<String, FieldConfig>>,
}

fn validate_path(dir: &Path) -> Result<()> {
    for component in dir.components() {
        if component.as_os_str() == ".." {
            return Err(Error::invalid_query(
                "path must not contain '..'",
            ));
        }
    }
    Ok(())
}

fn validate_collection_name(name: &str) -> Result<()> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(Error::invalid_query(format!(
            "invalid collection name '{}'",
            name
        )));
    }
    Ok(())
}

impl Engine {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        validate_path(&dir)?;

        let state_path = dir.join(STATE_FILE);

        if !state_path.exists() {
            let mut engine = Engine::new();
            engine.storage_dir = Some(dir);
            return Ok(engine);
        }

        let data = std::fs::read_to_string(&state_path).map_err(|e| Error::Store {
            msg: format!("failed to read state: {e}"),
        })?;

        let state: PersistedState = serde_json::from_str(&data).map_err(|e| Error::Store {
            msg: format!("corrupt state file: {e}"),
        })?;

        if state.version > CURRENT_VERSION {
            return Err(Error::Store {
                msg: format!(
                    "unsupported state version {} (current {})",
                    state.version, CURRENT_VERSION
                ),
            });
        }

        let mut engine = Engine::new();
        engine.storage_dir = Some(dir);

        for (collection, _cfgs) in &state.field_configs {
            validate_collection_name(collection)?;
        }

        engine.field_configs = state.field_configs;

        for (collection, docs) in &state.documents {
            let _cfgs = engine.field_configs.get(collection).ok_or_else(|| {
                Error::Store {
                    msg: format!(
                        "collection '{}' has documents but no field config",
                        collection
                    ),
                }
            })?;
            for (_, doc) in docs {
                engine.add_document(collection, doc.clone())?;
            }
        }

        Ok(engine)
    }

    pub fn save(&self) -> Result<()> {
        let dir = self.storage_dir.as_ref().ok_or_else(|| Error::Store {
            msg: "no storage directory set; use Engine::open()".to_string(),
        })?;

        std::fs::create_dir_all(dir).map_err(|e| Error::Store {
            msg: format!("failed to create storage dir: {e}"),
        })?;

        for collection in self.field_configs.keys() {
            validate_collection_name(collection)?;
        }

        let state = PersistedState {
            version: CURRENT_VERSION,
            documents: self.documents.clone(),
            field_configs: self.field_configs.clone(),
        };

        let data = serde_json::to_string_pretty(&state).map_err(|e| Error::Store {
            msg: format!("failed to serialize state: {e}"),
        })?;

        std::fs::write(dir.join(STATE_FILE), &data).map_err(|e| Error::Store {
            msg: format!("failed to write state: {e}"),
        })?;

        Ok(())
    }
}

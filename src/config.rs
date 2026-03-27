use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub members: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum DependencyConfig {
    Simple(String),
    Full {
        version: String,
        repo: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackConfig {
    pub mode: String, // "fat", "flat", "none"
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RuntimeConfig {
    pub vendor: String, // "temurin", "corretto", "zulu", "microsoft", "oracle"
    pub version: String,
    pub checksum: Option<String>,
    pub auto_verify: Option<bool>,
    pub accept_oracle_licence_terms: Option<bool>, // When using Oracle JDK, due to licensing requirements, you must explicitly set this option to true.
}

#[derive(Debug, Deserialize)]
pub struct SporaConfig {
    pub workspace: Option<WorkspaceConfig>,
    pub project: Option<Project>,
    pub toolchain: ToolchainConfig,
    pub dependencies: HashMap<String, DependencyConfig>,
    pub pack: Option<PackConfig>,
    pub runtime: Option<RuntimeConfig>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Project {
    pub name: String,
    pub lang: String,
    pub version: String,
    pub java_version: Option<String>,
    pub kotlin_version: Option<String>,
    pub main: String,
}

impl SporaConfig {
    pub fn load() -> Self {
        Self::load_from_path(std::path::Path::new("spora.toml"))
    }

    pub fn load_from_path(path: &std::path::Path) -> Self {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("❌ {:?} が見つかりません。", path));
        toml::from_str(&content).expect("❌ TOMLの解析に失敗しました。")
    }
}

#[derive(Debug, Deserialize)]
pub struct ToolchainConfig {
    pub java: String,
    pub kotlin: Option<String>,
}

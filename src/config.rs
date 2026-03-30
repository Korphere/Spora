use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use anyhow::{Context, Result};

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
    pub lang: String, // "java", "kotlin"
    pub vendor: String, // "temurin", "corretto", "zulu", "microsoft", "oracle"
    pub version: String,
    pub platform: Option<String>, /// "jvm", "native"
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
    pub fn load() -> Result<SporaConfig> {
        Self::load_from_path(std::path::Path::new("spora.toml"))
            .context("spora.tomlの読み込みに失敗しました")
    }

    pub fn load_from_path(path: &std::path::Path) -> Result<SporaConfig> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("{:?} が見つかりません", path))?;

        let toml: SporaConfig = toml::from_str(&content)
            .context("TOMLの解析に失敗しました")?;

        Ok(toml)
    }
}

#[derive(Debug, Deserialize)]
pub struct ToolchainConfig {
    pub java: String,
    pub kotlin: Option<String>,
}

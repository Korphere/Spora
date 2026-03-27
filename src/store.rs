use sha2::{Sha256, Digest};
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default)]
pub struct StoreIndex {
    // key: "group:artifact:version", value: "sha256_hash"
    pub entries: HashMap<String, String>,
}
pub struct SporaStore {
    pub root: PathBuf,
}

impl SporaStore {
    pub fn new() -> Self {
        let root = dirs::home_dir().unwrap().join(".spora/store");
        if !root.exists() {
            fs::create_dir_all(&root).unwrap();
        }
        SporaStore { root }
    }

    pub fn store_file(&self, path: &Path) -> String {
        let content = fs::read(path).expect("Failed to read file");
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hex::encode(hasher.finalize());

        let target_path = self.root.join(&hash);
        if !target_path.exists() {
            fs::copy(path, target_path).expect("Failed to copy file to store");
        }
        hash
    }

    pub fn get_path(&self, hash: &str) -> PathBuf {
        self.root.join(hash)
    }

    fn index_path(&self) -> PathBuf {
        self.root.parent().unwrap().join("index.json")
    }

    pub fn store_with_coords(&self, path: &Path, coords_str: &str) -> String {
        let hash = self.store_file(path);
        
        let mut index = self.load_index();
        index.entries.insert(coords_str.to_string(), hash.clone());
        self.save_index(index);
        
        hash
    }

    pub fn lookup(&self, coords_str: &str) -> Option<String> {
        self.load_index().entries.get(coords_str).cloned()
    }

    fn load_index(&self) -> StoreIndex {
        let path = self.index_path();
        if !path.exists() { return StoreIndex::default(); }
        let content = fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    }

    fn save_index(&self, index: StoreIndex) {
        let content = serde_json::to_string_pretty(&index).unwrap();
        fs::write(self.index_path(), content).ok();
    }
}
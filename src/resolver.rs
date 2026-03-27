use std::fs;
use std::path::{Path};
use std::sync::Mutex;
use std::collections::HashSet;
use crate::store::SporaStore;
use roxmltree::Document;
use serde::{Deserialize, Serialize};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LockedDependency {
    pub coords: String,
    pub hash: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct LockFile {
    pub dependencies: Vec<LockedDependency>,
}

pub struct Resolver {
    store: SporaStore,
    resolved_cache: Mutex<HashSet<String>>,
    multi_progress: MultiProgress,
}

impl Resolver {
    pub fn new() -> Self {
        Resolver { 
            store: SporaStore::new(),
            resolved_cache: Mutex::new(HashSet::new()),
            multi_progress: MultiProgress::new(),
        }
    }

    pub fn resolve_recursive(&self, coords: &MavenCoords, repo_override: Option<&String>, project_root: &Path) {
        let identifier = format!("{}:{}:{}", coords.group.replace('/', "."), coords.artifact, coords.version);
        
        {
            let mut cache = self.resolved_cache.lock().unwrap();
            if cache.contains(&identifier) { return; }
            cache.insert(identifier);
        }

        let jar_display_name = format!("{}-{}", coords.artifact, coords.version);
        let lib_path = project_root.join("lib").join(format!("{}.jar", jar_display_name));

        if lib_path.exists() {
            println!("{} is already in lib. Skipping download.", jar_display_name);
            return; 
        }

        println!("Resolving: {}:{}", coords.artifact, coords.version);

        self.fetch_dependencies_from_pom(coords, repo_override, project_root);

        self.download_and_link_jar(coords, repo_override, project_root);
    }

    fn fetch_dependencies_from_pom(&self, coords: &MavenCoords, repo_override: Option<&String>, project_root: &Path) {
        let default_repo = "https://repo1.maven.org/maven2/".to_string();
        let base_url = repo_override.unwrap_or(&default_repo).trim_end_matches('/');

        let pom_url = format!(
            "{}/{}/{}/{}/{}-{}.pom",
            base_url, coords.group, coords.artifact, coords.version, coords.artifact, coords.version
        );

        if let Ok(response) = reqwest::blocking::get(&pom_url) {
            let xml_text = response.text().unwrap_or_default();
            if let Ok(doc) = Document::parse(&xml_text) {
                let deps = doc.descendants()
                    .filter(|n| n.has_tag_name("dependency"));
                for dep in deps {
                    let g = dep.children().find(|n| n.has_tag_name("groupId")).map(|n| n.text()).flatten();
                    let a = dep.children().find(|n| n.has_tag_name("artifactId")).map(|n| n.text()).flatten();
                    let v = dep.children().find(|n| n.has_tag_name("version")).map(|n| n.text()).flatten();
                    let scope = dep.children().find(|n| n.has_tag_name("scope")).map(|n| n.text()).flatten();

                    if let (Some(g), Some(a), Some(v)) = (g, a, v) {
                        if scope != Some("test") && scope != Some("provided") {
                            let next_coords = MavenCoords::new(g, a, v);
                            self.resolve_recursive(&next_coords, repo_override, project_root);
                        }
                    }
                }
            }
        }
    }

    fn download_and_link_jar(&self, coords: &MavenCoords, repo_override: Option<&String>, project_root: &Path) {
        let coords_str = format!("{}:{}:{}", coords.group, coords.artifact, coords.version);
        let lib_dir = project_root.join("lib");
        let display_name = format!("{}-{}", coords.artifact, coords.version);
        let dest = lib_dir.join(format!("{}.jar", display_name));

        if let Some(hash) = self.store.lookup(&coords_str) {
            let src = self.store.get_path(&hash);
            if src.exists() {
                self.link_or_copy(&src, &dest);
                return;
            }
        }

        let pb = self.multi_progress.add(ProgressBar::new_spinner());
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] 📥 {msg}")
            .unwrap());
        pb.set_message(format!("Fetching {}...", display_name));
        pb.enable_steady_tick(std::time::Duration::from_millis(120));

        let target_repos = if let Some(url) = repo_override {
            vec![url.clone()]
        } else {
            vec!["https://repo1.maven.org/maven2/".to_string()]
        };

        for base_url in target_repos {
            let jar_url = format!(
                "{}/{}/{}/{}/{}-{}.jar",
                base_url.trim_end_matches('/'),
                coords.group, coords.artifact, coords.version, coords.artifact, coords.version
            );

            if let Ok(response) = reqwest::blocking::get(&jar_url) {
                if response.status().is_success() {
                    let bytes = response.bytes().expect("Failed to read JAR bytes");
                    let temp_dir = std::env::temp_dir();
                    let temp_filename = format!("spora_{}.jar", coords.artifact);
                    let temp_path = temp_dir.join(temp_filename);
                    
                    fs::write(&temp_path, &bytes).unwrap();

                    let hash = self.store.store_with_coords(&temp_path, &coords_str);
                    
                    let _ = fs::remove_file(temp_path);

                    let src = self.store.get_path(&hash);
                    self.link_or_copy(&src, &dest);
                    
                    println!("Linked to store: {}", dest.display());
                    println!("Downloaded from: {}", base_url);
                    pb.finish_with_message(format!("Resolved {}", display_name));
                    return;
                } else {
                    eprintln!("Failed to download JAR: {} (Status: {})", jar_url, response.status());
                }
            }
        }
        pb.finish_with_message(format!("Failed {}", display_name));
    }

    fn link_or_copy(&self, src: &Path, dest: &Path) {
        fs::create_dir_all(dest.parent().unwrap()).ok();
        if !dest.exists() {
            if std::fs::hard_link(src, dest).is_err() {
                fs::copy(src, dest).ok();
            }
        }
    }

    pub fn save_lock(&self, project_root: &std::path::Path) {
        let mut lock = LockFile::default();
        let cache = self.resolved_cache.lock().unwrap();
        
        for identifier in cache.iter() {
            if let Some(hash) = self.store.lookup(identifier) {
                lock.dependencies.push(LockedDependency {
                    coords: identifier.clone(),
                    hash,
                });
            }
        }

        let content = serde_json::to_string_pretty(&lock).unwrap();
        fs::write(project_root.join("spora.lock"), content).expect("Failed to write spora.lock");
        println!("Created spora.lock");
    }

    pub fn resolve_from_lock(&self, project_root: &std::path::Path) -> bool {
        let lock_path = project_root.join("spora.lock");
        if !lock_path.exists() { return false; }

        println!("Found spora.lock. Restoring dependencies...");
        let content = fs::read_to_string(lock_path).unwrap();
        let lock: LockFile = serde_json::from_str(&content).unwrap();

        for dep in lock.dependencies {
            let hash = dep.hash;
            let src = self.store.get_path(&hash);
            
            let parts: Vec<&str> = dep.coords.split(':').collect();
            if parts.len() == 3 {
                let dest = project_root.join("lib").join(format!("{}-{}.jar", parts[1], parts[2]));
                self.link_or_copy(&src, &dest);
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct MavenCoords {
    pub group: String,
    pub artifact: String,
    pub version: String,
}

impl MavenCoords {
    pub fn new(g: &str, a: &str, v: &str) -> Self {
        MavenCoords {
            group: g.replace('.', "/"),
            artifact: a.to_string(),
            version: v.to_string(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 3 {
            Some(MavenCoords::new(parts[0], parts[1], parts[2]))
        } else { None }
    }
}
use std::{collections::HashMap, process::Command};
use std::fs;
use crate::logger::Logger;
use crate::toolchain::Toolchain;
use std::path::{Path};
use walkdir::WalkDir;
use sha2::{Sha256, Digest};

use crate::config::SporaConfig;
pub struct Builder;

impl Builder {
    pub fn bloom(base_path: &Path, project_name: &str, lang: &str, _java_ver: &str, _kotlin_ver: &str, main_class: &str) {
        Logger::log_step("Compiling", project_name);

        let lang_dir = match lang {
            "java" => "java",
            "kotlin" => "kt",
            _ => "unknown",
        };
        let source_dir = base_path.join("src").join("main").join(lang_dir);
        let resource_dir = base_path.join("src").join("main").join("resources");
        let lib_dir = base_path.join("lib");
        let out_dir = base_path.join("out");
        let _fingerprint_path = base_path.join(".spora/fingerprints.json");

        Self::copy_resources(&resource_dir, &out_dir);

        let mut lib_fingerprints = HashMap::new();

        let mut classpath_elements = Vec::new();

        // 2. libディレクトリ内のJARを追加
        if let Ok(entries) = fs::read_dir(&lib_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jar") {
                    classpath_elements.push(path.to_string_lossy().to_string());
                    
                    let lib_name = path.file_name().unwrap().to_str().unwrap().to_string();
                    let hash = Self::calculate_hash(&path);
                    lib_fingerprints.insert(format!("lib:{}", lib_name), hash);
                }
            }
        }

        classpath_elements.push(out_dir.to_string_lossy().to_string());

        let separator = if cfg!(target_os = "windows") { ";" } else { ":" };
        let classpath = classpath_elements.join(separator);

        fs::create_dir_all(out_dir.clone()).ok();
        
        let fingerprints = Self::load_fingerprints(base_path);
        let mut new_fingerprints = HashMap::new();
        let mut needs_full_rebuild = false;

        for (key, hash) in &lib_fingerprints {
            if fingerprints.get(key) != Some(hash) {
                Logger::log_info("Changed", &format!("dependency: {}", key));
                needs_full_rebuild = true;
            }
        }

        let root_config = SporaConfig::load();
        let runtime = root_config.runtime.as_ref().expect("Runtime config is required");
        let compiler_path = match lang {
            "java" => Toolchain::get_compiler_path("java", &runtime),
            "kotlin" => Toolchain::get_compiler_path("kotlin", &runtime),
            _ => panic!("Unsupported language"),
        };

        let mut source_files = Vec::new();
        let extension = if lang == "kotlin" { "kt" } else { "java" };
        
        if source_dir.exists() {
            for entry in WalkDir::new(&source_dir).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                
                if path.is_file() && path.extension().map_or(false, |ext| ext == extension) {
                    let path_str = path.to_str().unwrap().to_string();
                    let current_hash = Self::calculate_hash(path);
                    
                    let relative_path = path.strip_prefix(&source_dir).expect("Failed to strip prefix");
                    let class_file = out_dir.join(relative_path).with_extension("class");
                    
                    if needs_full_rebuild || fingerprints.get(&path_str) != Some(&current_hash) || !class_file.exists() {
                        source_files.push(path_str.clone());
                    }
                    
                    new_fingerprints.insert(path_str, current_hash);
                }
            }
        }

        for (k, v) in lib_fingerprints {
            new_fingerprints.insert(k, v);
        }

        if source_files.is_empty() {
            Logger::log_success("Everything is up to date.");
            return;
        }

        println!("Compiling {} changed file(s)...", source_files.len());

        let mut cmd = Command::new(compiler_path);
        cmd.arg("-cp").arg(classpath).arg("-d").arg(&out_dir);
        
        for file in source_files {
            cmd.arg(file);
        }

        let status = cmd.status().expect("❌ コンパイラの実行に失敗しました。");

        if status.success() {
            Logger::log_success("Compilation successful");
            Self::save_fingerprints(base_path, new_fingerprints);
            Self::package(project_name, out_dir.clone().to_str().unwrap(), main_class);
        }
    }

    fn package(name: &str, out_dir: &str, main_class: &str) {
        Logger::log_step("Packaging", &format!("{}.jar", name));
        
        let jar_file = format!("{}.jar", name);

        // jar --create --file <name>.jar --main-class <main> -C <dir> .
        let status = Command::new("jar")
            .arg("cvfe")
            .arg(&jar_file)
            .arg(main_class)
            .arg("-C")
            .arg(out_dir)
            .arg(".")
            .status()
            .expect("❌ JARの作成に失敗しました。");

        if status.success() {
            Logger::log_success(&format!("Created {}.jar (Main: {})", name, main_class));
        }
    }

    fn calculate_hash(path: &Path) -> String {
        let content = fs::read(path).expect("Failed to read source file");
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    fn load_fingerprints(base_path: &Path) -> HashMap<String, String> {
        let path = base_path.join(".spora/fingerprints.json");
        if !path.exists() { return HashMap::new(); }
        let content = fs::read_to_string(path).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str(&content).unwrap_or_else(|_| HashMap::new())
    }

    fn save_fingerprints(base_path: &Path, fingerprints: HashMap<String, String>) {
        let dot_spora = base_path.join(".spora");
        fs::create_dir_all(&dot_spora).ok();
        let content = serde_json::to_string_pretty(&fingerprints).unwrap();
        fs::write(dot_spora.join("fingerprints.json"), content).expect("Failed to save fingerprints");
    }

    fn copy_resources(res_src_dir: &Path, out_dir: &Path) {
        if !res_src_dir.exists() { return; }
        
        Logger::log_step("Copying", "resources...");

        for entry in WalkDir::new(res_src_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(res_src_dir).unwrap();
                let target_path = out_dir.join(relative_path);

                fs::create_dir_all(target_path.parent().unwrap()).ok();
                fs::copy(path, &target_path).ok();
            }
        }
    }

    pub fn is_lock_unchanged(project_root: &Path) -> bool {
        let lock_path = project_root.join("spora.lock");
        if !lock_path.exists() {
            return false;
        }

        let current_hash = Self::hash_file(&lock_path);

        let hash_storage = project_root.join(".spora/lock.hash");
        if hash_storage.exists() {
            let last_hash = fs::read_to_string(&hash_storage).unwrap_or_default();
            if current_hash == last_hash {
                return true;
            }
        }

        fs::create_dir_all(project_root.join(".spora")).ok();
        fs::write(hash_storage, current_hash).ok();
        false
    }

    pub fn hash_file(path: &Path) -> String {
        let content = fs::read(path).expect("❌ Failed to read file for hashing");
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }
}
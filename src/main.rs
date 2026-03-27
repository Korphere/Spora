mod config;
mod store;
mod resolver;
mod builder;
mod toolchain;
mod logger;

use config::SporaConfig;
use resolver::{Resolver, MavenCoords};
use std::{env, fs, path::Path};
use std::sync::Arc;
use std::sync::Mutex;
use std::io::{Read, Write};
use builder::Builder;
use zip::ZipArchive;
use rayon::prelude::*;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use colored::*;

use crate::config::DependencyConfig;
use crate::logger::Logger;

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match command {
        "init" => {
            fs::create_dir_all("src/main/java").ok();
            fs::create_dir_all("src/main/resources").ok();
            fs::create_dir_all("lib").ok();

            let default_config = r#"[project]
name = "my-spora-project"
lang = "java"
version = "0.1.0"
main = "Main"

[toolchain]
java = "21"

[runtime]
vendor = "temurin"
version = "21"

[dependencies]
"#;
            fs::write("spora.toml", default_config).unwrap();

            let hello_java = r#"public class Main {
    public static void main(String[] args) {
        System.out.println("Hello from Spora!");
    }
}"#;
            fs::write("src/main/java/Main.java", hello_java).unwrap();

            Logger::log_success("Project initialized in standard layout");
        },
        "fetch" => {
            Logger::log_step("Fetching", "dependencies...");
            let root_config = SporaConfig::load();
            let resolver = Resolver::new();
            let project_root = std::env::current_dir().unwrap();
            perform_fetch(&root_config, &resolver, &project_root);
        },
        "build" => {
            let root_config = SporaConfig::load();

            if let Some(runtime) = &root_config.runtime {
                if runtime.vendor == "oracle" && !runtime.accept_oracle_licence_terms.unwrap_or(false) {
                    Logger::log_error("Oracle JDK requires license acceptance.");
                    println!("             Please set 'accept_oracle_licence_terms = true' in spora.toml");
                    std::process::exit(1);
                }
                Logger::log_info("Runtime", &format!("Using {} JDK v{}", runtime.vendor, runtime.version));
            }

            Logger::log_step("Building", "project modules...");
            perform_build(&root_config);
        },
        "bloom" => {
            let root_config = SporaConfig::load();

            if let Some(runtime) = &root_config.runtime {
                if runtime.vendor == "oracle" && !runtime.accept_oracle_licence_terms.unwrap_or(false) {
                    Logger::log_error("Oracle JDK requires license acceptance.");
                    println!("             Please set 'accept_oracle_licence_terms = true' in spora.toml");
                    std::process::exit(1);
                }
                Logger::log_info("Runtime", &format!("Using {} JDK v{}", runtime.vendor, runtime.version));
            }

            let resolver = Resolver::new();
            let project_root = std::env::current_dir().unwrap();

            Logger::log_step("Fetching", "dependencies...");
            perform_fetch(&root_config, &resolver, &project_root);
            Logger::log_step("Compiling", "project...");
            perform_build(&root_config);
            
            Logger::log_success("Spora has bloomed");
        },
        "run" => {
            let config = SporaConfig::load();
            let runtime = config.runtime.as_ref().expect("Runtime config is required");
            let java_exe = toolchain::Toolchain::get_compiler_path("java", runtime)
                .parent().unwrap()
                .join(if cfg!(windows) { "java.exe" } else { "java" });

            let project = config.project.as_ref().expect("Project configuration is missing in spora.toml");
            
            let mut classpath = String::from("out");
            let sep = if cfg!(windows) { ";" } else { ":" };

            if let Ok(entries) = fs::read_dir("lib") {
                for entry in entries.filter_map(|e| e.ok()) {
                    if entry.path().extension().map_or(false, |ext| ext == "jar") {
                        classpath.push_str(sep);
                        classpath.push_str(entry.path().to_str().unwrap());
                    }
                }
            }

            Logger::log_step("Running", &format!("{}...", project.name));

            let status = std::process::Command::new(java_exe)
                .arg("-cp")
                .arg(classpath) 
                .arg(&project.main)
                .status()
                .expect("❌ 実行に失敗しました。");

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        },
        "clean" => {
            Logger::log_info("Cleaning", "target directories");
            
            let targets = vec!["out", "out_fat", ".spora", "lib"];
            
            for target in targets {
                let path = std::path::Path::new(target);
                if path.exists() {
                    if path.is_dir() {
                        std::fs::remove_dir_all(path).expect("❌ Failed to remove directory");
                    } else {
                        std::fs::remove_file(path).expect("❌ Failed to remove file");
                    }

                    println!("          {} {}", "Removed:".red(), target);
                }
            }
            
            if let Ok(_content) = std::fs::read_to_string("spora.toml") {
                let config = SporaConfig::load();
                let project = config.project.as_ref().expect("Project configuration is missing");
                let jar_name = format!("{}.jar", project.name);
                if std::path::Path::new(&jar_name).exists() {
                    std::fs::remove_file(&jar_name).ok();
                    println!("          {} {}", "Removed:".red(), jar_name);
                }
            }

            Logger::log_success("Project cleaned successfully");
        },
        _ => println!("Usage: spora {}",  "[init | fetch | build | run | bloom | clean]".bold()),
    }
}

fn resolve_deps(config: &SporaConfig, resolver: &Resolver, path: &Path) {
    config.dependencies.par_iter().for_each(|(name, dep)| {
        let (version, repo_url) = match dep {
            DependencyConfig::Simple(v) => (v.clone(), None),
            DependencyConfig::Full { version, repo } => (version.clone(), repo.as_ref()),
        };

        let full_coords = format!("{}:{}", name, version);
        if let Some(coords) = MavenCoords::parse(&full_coords) {
            resolver.resolve_recursive(&coords, repo_url, path);
        }
    });
}

fn perform_fetch(root_config: &SporaConfig, resolver: &Resolver, project_root: &Path) {
    Logger::log_step("Fetching", "dependencies...");
    if resolver.resolve_from_lock(project_root) {
        Logger::log_success("Dependencies restored from spora.lock");
    } else {
        if let Some(ws) = &root_config.workspace {
            for member_name in &ws.members {
                let member_path = Path::new(member_name);
                let config_path = member_path.join("spora.toml");
                if config_path.exists() {
                    let m_config = SporaConfig::load_from_path(&config_path);
                    resolve_deps(&m_config, resolver, member_path);
                }
            }
        } else {
            resolve_deps(root_config, resolver, project_root);
        }
        resolver.save_lock(project_root);
        Logger::log_success("Dependencies resolved and locked");
    }
}

fn perform_build(root_config: &SporaConfig) {
    Logger::log_step("Building", "project...");
    
    if let Some(ws) = &root_config.workspace {
        ws.members.par_iter().for_each(|member_name| {
            let member_path = Path::new(member_name);
            let config_path = member_path.join("spora.toml");
            
            if config_path.exists() {
                let m_config = SporaConfig::load_from_path(&config_path);
                if let Some(p) = &m_config.project {
                    build_module(member_path, p, root_config);
                }
            }
        });
    } else if let Some(p) = &root_config.project {
        build_module(Path::new("."), p, root_config);
    }
}

fn build_module(path: &Path, project: &crate::config::Project, root_config: &SporaConfig) {
    Builder::bloom(
        path,
        &project.name,
        &project.lang,
        &root_config.toolchain.java,
        root_config.toolchain.kotlin.as_deref().unwrap_or("1.9.22"),
        &project.main
    );

    let pack_mode = root_config.pack.as_ref()
        .map(|p| p.mode.as_str())
        .unwrap_or("none");

    Logger::log_step("Packaging", &format!("mode: {}", pack_mode));

    let lock = Arc::new(Mutex::new(()));
    match pack_mode {
        "fat" => create_fat_jar(path, project, root_config, Arc::clone(&lock)),
        "flat" => create_flat_jar(path, project, root_config),
        "none" | _ => create_simple_jar(path, project, root_config),
    }
}

fn create_simple_jar(path: &Path, project: &config::Project, root_config: &SporaConfig) {
    let jar_name = format!("{}.jar", project.name);
    Logger::log_step("Packaging", &format!("Simple JAR: {}", jar_name));

    let runtime = root_config.runtime.as_ref().expect("Runtime config is required");
    let jar_exe = toolchain::Toolchain::get_compiler_path("java", runtime)
        .parent().unwrap()
        .join(if cfg!(windows) { "jar.exe" } else { "jar" });

    let status = std::process::Command::new(jar_exe)
        .arg("cfe")
        .arg(&jar_name)
        .arg(&project.main)
        .arg("-C")
        .arg(path.join("out"))
        .arg(".")
        .status()
        .expect("❌ JARの作成に失敗しました。");

    if status.success() {
        Logger::log_success(&format!("Build successful: {}", jar_name));
    }
}

fn create_flat_jar(path: &Path, project: &config::Project, root_config: &SporaConfig) {
    let jar_name = format!("{}.jar", project.name);
    Logger::log_step("Packaging", &format!("Flat JAR: {} (with Class-Path)", jar_name));

    let mut classpath_attr = String::new();
    let lib_dir = path.join("lib");
    if let Ok(entries) = fs::read_dir(&lib_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().extension().map_or(false, |ext| ext == "jar") {
                let file_name = entry.file_name().into_string().unwrap();
                classpath_attr.push_str(&format!("lib/{} ", file_name));
            }
        }
    }
    let classpath_attr = classpath_attr.trim();

    let manifest_content = format!(
        "Manifest-Version: 1.0\nMain-Class: {}\nClass-Path: {}\n\n",
        project.main, classpath_attr
    );
    let manifest_path = path.join("MANIFEST.MF");
    fs::write(&manifest_path, manifest_content).expect("❌ マニフェストの作成に失敗しました。");

    let runtime = root_config.runtime.as_ref().expect("Runtime config is required");
    let jar_exe = toolchain::Toolchain::get_compiler_path("java", runtime)
        .parent().unwrap()
        .join(if cfg!(windows) { "jar.exe" } else { "jar" });

    let status = std::process::Command::new(jar_exe)
        .arg("cvfm")
        .arg(&jar_name)
        .arg(&manifest_path)
        .arg("-C")
        .arg(path.join("out"))
        .arg(".")
        .status()
        .expect("❌ JARの作成に失敗しました。");

    fs::remove_file(manifest_path).ok();

    if status.success() {
        Logger::log_success(&format!("Build Successful: {}", jar_name));
        Logger::log_info("Note","To run this, you need the 'lib/' directory next to the JAR");
    }
}

fn create_fat_jar(path: &Path, project: &config::Project, root_config: &SporaConfig, fs_lock: Arc<Mutex<()>>) {
    let jar_name = format!("{}.jar", project.name);
    let temp_unpack = path.join("out_fat");
    
    let lock_unchanged = Builder::is_lock_unchanged(path);
    let cache_exists = temp_unpack.exists();

    if lock_unchanged && cache_exists {
        Logger::log_step("Skipping", "re-unpacking");
    } else {
        if temp_unpack.exists() {
            fs::remove_dir_all(&temp_unpack).ok();
        }
        fs::create_dir_all(&temp_unpack).expect("❌ 一時ディレクトリの作成に失敗しました。");

        let lib_dir = path.join("lib");
        if let Ok(entries) = fs::read_dir(&lib_dir) {
            let jar_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "jar"))
                .collect();

            let mp = MultiProgress::new();
            let pb = mp.add(ProgressBar::new(jar_files.len() as u64));
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"));
            
            pb.set_message("Unpacking dependencies...");

            jar_files.par_iter().for_each(|entry| {
                let file_name = entry.file_name().into_string().unwrap_or_default();
                pb.set_message(format!("Extracting {}", file_name));
                
                unpack_jar(&entry.path(), &temp_unpack, Arc::clone(&fs_lock));
                
                pb.inc(1);
            });
            
            pb.finish_with_message("All dependencies unpacked!");
        }
    }

    println!("  📂 Merging project classes...");
    let out_dir = path.join("out");
    if out_dir.exists() {
        copy_dir_recursive(&out_dir, &temp_unpack);
    }

    let runtime = root_config.runtime.as_ref().expect("Runtime config is required");
    let jar_exe = toolchain::Toolchain::get_compiler_path("java", runtime)
        .parent().unwrap()
        .join(if cfg!(windows) { "jar.exe" } else { "jar" });

    let status = std::process::Command::new(jar_exe)
        .arg("cfe")
        .arg(&jar_name)
        .arg(&project.main)
        .arg("-C")
        .arg(&temp_unpack)
        .arg(".")
        .status()
        .expect("❌ JARの作成に失敗しました。");

    fs::remove_dir_all(&temp_unpack).ok();

    if status.success() {
        Logger::log_success(&format!("Fat JAR created: {}", jar_name));
        Logger::log_hint(&format!("This JAR is self-contained and can be run with 'java -jar {}'", jar_name));
    }
}

fn unpack_jar(jar_path: &Path, dest_dir: &Path, fs_lock: Arc<Mutex<()>>) {
    let file = fs::File::open(jar_path).expect("❌ JARファイルが開けません");
    let mut archive = ZipArchive::new(file).expect("❌ 不正なJAR形式です");

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        let name = file.name();
        
        if name.starts_with("META-INF/") && (name.ends_with(".SF") || name.ends_with(".DSA") || name.ends_with(".RSA")) {
            continue;
        }

        if (*file.name()).ends_with('/') {
            fs::create_dir_all(&outpath).ok();
        } else {
            if let Some(p) = outpath.parent() { fs::create_dir_all(&p).ok(); }

            let _guard = fs_lock.lock().unwrap();

            if outpath.exists() {
                if name.starts_with("META-INF/services/") {
                    let mut existing_content = Vec::new();
                    fs::File::open(&outpath).unwrap().read_to_end(&mut existing_content).ok();
                    
                    let mut new_content = Vec::new();
                    file.read_to_end(&mut new_content).ok();

                    if !existing_content.windows(new_content.len()).any(|w| w == new_content) {
                        let mut outfile = fs::OpenOptions::new()
                            .append(true)
                            .open(&outpath)
                            .unwrap();
                        writeln!(outfile).ok();
                        outfile.write_all(&new_content).ok();
                    }
                    continue;
                }
                
                continue; 
            }

            let mut outfile = fs::File::create(&outpath).unwrap();
            std::io::copy(&mut file, &mut outfile).ok();
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    for entry in walkdir::WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel_path = entry.path().strip_prefix(src).unwrap();
        let target_path = dst.join(rel_path);
        if entry.path().is_dir() {
            fs::create_dir_all(&target_path).ok();
        } else {
            fs::copy(entry.path(), &target_path).ok();
        }
    }
}
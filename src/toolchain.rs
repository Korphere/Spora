use std::fs;
use std::path::{Path, PathBuf};
use serde_json::Value;
use zip::ZipArchive;
use flate2::read::GzDecoder;
use tar::Archive;
use crate::config::RuntimeConfig;
use crate::logger::Logger;
use std::io::{self, Seek, SeekFrom};
pub struct Toolchain;

impl Toolchain {
    pub fn get_compiler_path(lang: &str, runtime: &RuntimeConfig) -> PathBuf {
        let version = &runtime.version;
        let vendor = &runtime.vendor;
        let tool_dir = dirs::home_dir().unwrap()
            .join(".spora")
            .join("tools")
            .join(lang)
            .join(vendor)
            .join(version);
        
        let bin_exists = tool_dir.join("bin").exists() || 
                        fs::read_dir(&tool_dir).ok().map(|rd| {
                            rd.filter_map(|e| e.ok()).any(|e| e.path().join("bin").exists())
                        }).unwrap_or(false);

        if !bin_exists {
            if tool_dir.exists() {
                let _ = fs::remove_dir_all(&tool_dir);
            }
            Logger::log_step("Setup", &format!("{} v{} is missing. Starting auto-setup...", lang, version));
            Self::setup_tool(lang, runtime, &tool_dir);
        }

        let actual_home = if tool_dir.join("bin").exists() {
            tool_dir
        } else {
            fs::read_dir(&tool_dir)
                .expect("Failed to read tool directory")
                .filter_map(|e| e.ok())
                .find(|e| e.path().is_dir())
                .map(|e| e.path())
                .unwrap_or(tool_dir)
        };

        let binary_name = match lang {
            "java" => if cfg!(windows) { "bin/javac.exe" } else { "bin/javac" },
            "kotlin" => if cfg!(windows) { "bin/kotlinc.bat" } else { "bin/kotlinc" },
            _ => panic!("Unsupported language"),
        };

        actual_home.join(binary_name)
    }

    fn setup_tool(lang: &str, runtime: &RuntimeConfig, target_dir: &Path) {
        fs::create_dir_all(target_dir).expect("Failed to create tool directory");

        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build HTTP client");

        let os = if cfg!(target_os = "windows") { "windows" } 
                    else if cfg!(target_os = "macos") { "mac" } 
                    else { "linux" };
        
        let arch = if cfg!(target_arch = "x86_64") { "x64" } else { "aarch64" };
        let _ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };

        let vendor = runtime.vendor.as_str();

        let major_version = runtime.version.split('.').next().unwrap_or("21");

        let url = match (lang, vendor) {
            ("java", "temurin") => {
                format!(
                    "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/adoptium",
                    major_version,
                    os,
                    arch
                )
            },
            ("java", "corretto") => {
                let corretto_os = match os {
                    "windows" => "x64-windows-jdk.zip",
                    "mac" => if arch == "x64" { "x64-macos-jdk.tar.gz" } else { "aarch64-macos-jdk.tar.gz" },
                    _ => "x64-linux-jdk.tar.gz",
                };
                format!("https://corretto.aws/downloads/latest/amazon-corretto-{}-{}", major_version, corretto_os)
            },
            ("java", "oracle") => {
                if !runtime.accept_oracle_licence_terms.unwrap_or(false) {
                    Logger::log_error("Oracle JDK requires license acceptance.");
                    println!("             Please set 'accept_oracle_licence_terms = true' in spora.toml");
                    std::process::exit(1);
                }
                let oracle_os = match os {
                    "windows" => "windows-x64_bin.zip",
                    "mac" => "macos-aarch64_bin.tar.gz",
                    _ => "linux-x64_bin.tar.gz",
                };
                format!("https://download.oracle.com/java/{}/archive/jdk-{}_{}", 
                    major_version, runtime.version, oracle_os)
            },
            ("java", v) => {
                let catalog = Self::fetch_catalog().expect("Failed to fetch remote version catalog. Check your internet connection.");
                catalog["java"][v][major_version][os][arch]
                    .as_str()
                    .expect(&format!("Version {} for {} on {}/{} is not defined in Spora catalog.", major_version, v, os, arch))
                    .to_string()
            },
            ("kotlin", _) => {
                format!("https://github.com/JetBrains/kotlin/releases/download/v{}/kotlin-compiler-{}.zip", 
                    runtime.version, runtime.version)
            },
            _ => panic!("Vendor {} for {} is not supported.", runtime.vendor, lang),
        };

        Logger::log_step("Download", &url);
        //let mut response = reqwest::blocking::get(&url).expect("Download failed");
        let mut response = client.get(&url).send().expect("Download failed");

        let final_url = response.url().as_str().to_string();

        if !response.status().is_success() {
            panic!("Failed to download tool: Status {}", response.status());
        }

        let mut tmp_file = tempfile::tempfile().expect("Failed to create temp file");
        io::copy(&mut response, &mut tmp_file).expect("Failed to save download");

        tmp_file.seek(SeekFrom::Start(0)).expect("Failed to seek to start of temp file");

        if final_url.contains(".zip") || cfg!(windows) {
            Logger::log_step("Extract", "Unzipping...");
            let mut zip = ZipArchive::new(tmp_file).expect("Invalid zip header - the downloaded file might be corrupted or an error page");
            for i in 0..zip.len() {
                let mut file = zip.by_index(i).unwrap();
                let enclosed_path = file.enclosed_name().ok_or("Invalid file path in ZIP").unwrap();
                let outpath = target_dir.join(enclosed_path);
                println!("Extracting to: {:?}", outpath);
                if let Some(p) = outpath.parent() { 
                    fs::create_dir_all(&p).map_err(|e| println!("Dir error: {:?}", e)).ok(); 
                }
                if (*file.name()).ends_with('/') {
                    // If directory
                    fs::create_dir_all(&outpath).ok();
                } else {
                    // If file
                    if let Some(p) = outpath.parent() {
                        fs::create_dir_all(&p).ok();
                    }
                    
                    let mut outfile = fs::File::create(&outpath).expect(&format!("Failed to create file: {:?}", outpath));
                    io::copy(&mut file, &mut outfile).expect("Failed to copy data to file");
                    
                    outfile.sync_all().ok();
                }
            }
        } else if final_url.contains(".tar.gz") || final_url.contains(".tgz") {
            Logger::log_step("Extract", "Unpacking tar.gz...");
            let tar_gz = GzDecoder::new(tmp_file);
            let mut archive = Archive::new(tar_gz);
            archive.unpack(target_dir).expect("Failed to unpack tar.gz");
        }else {
            panic!("Unknown file format from URL: {}", final_url);
        }
        Logger::log_success("Setup complete");
    }

    fn fetch_catalog() -> Result<Value, Box<dyn std::error::Error>> {
        let catalog_url = "https://raw.githubusercontent.com/Korphere/spora/main/resources/versions.json";
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        
        let res = client.get(catalog_url).send()?;
        let json: Value = res.json()?;
        Ok(json)
    }
}
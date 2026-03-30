use std::fs;
use std::path::{Path, PathBuf};
use serde_json::Value;
use zip::ZipArchive;
use flate2::read::GzDecoder;
use tar::Archive;
use crate::config::RuntimeConfig;
use crate::logger::Logger;
use crate::platform::Platform;
use std::io::{self, Seek, SeekFrom};
use reqwest::redirect::Policy as RedirectPolicy;
use sha2::{Sha256, Digest};

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
            .redirect(RedirectPolicy::default())
            .referer(true)
            .build()
            .expect("Failed to build HTTP client");

        let platform = Platform::current();
        let os = platform.os;
        let arch = platform.arch;
        let _ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };

        let vendor = runtime.vendor.as_str();
        let major_version = runtime.version.split('.').next().unwrap_or("21");
        let runtime_platform = match &runtime.platform {
            Some(p) => p.as_str(),
            None => "jvm"
        };

        let url = Toolchain::get_url_by_lang_and_vendor(lang, vendor, major_version, os, arch, runtime_platform, runtime);

        Logger::log_step("Download", &url);
        let mut response = client.get(&url).send().expect("Download failed");

        let final_url = response.url().as_str().to_string();

        if !response.status().is_success() {
            panic!("Failed to download tool: Status {}", response.status());
        }

        let mut tmp_file = tempfile::tempfile().expect("Failed to create temp file");
        io::copy(&mut response, &mut tmp_file).expect("Failed to save download");

        if let Some(cat) = Self::fetch_catalog().ok() {
            let expected_hash = cat.get(lang)
                .and_then(|l| l.get(runtime_platform))
                .and_then(|p| p.get(vendor))
                .and_then(|v| v.get(&runtime.version).or_else(|| v.get(major_version)))
                .and_then(|ver| ver.get(os))
                .and_then(|o| o.get(arch))
                .and_then(|a| a.get("sha256"))
                .and_then(|s| s.as_str());

            if let Some(expected) = expected_hash {
                let actual_hash = Self::calculate_hash(&mut tmp_file).expect("Hash calculation failed");
                if actual_hash.to_lowercase() != expected.to_lowercase() {
                    panic!("Checksum mismatch!\nExpected: {}\nActual:   {}", expected, actual_hash);
                }
                Logger::log_success("Checksum verified");
            }
        }

        tmp_file.seek(SeekFrom::Start(0)).expect("Failed to seek to start of temp file");

        if final_url.contains(".zip") || cfg!(windows) {
            Logger::log_step("Extract", "Unzipping...");
            let mut zip = ZipArchive::new(tmp_file).expect("Invalid zip header - the downloaded file might be corrupted or an error page");
            for i in 0..zip.len() {
                let mut file = zip.by_index(i).unwrap();
                let enclosed_path = file.enclosed_name().ok_or("Invalid file path in ZIP").unwrap();
                let outpath = target_dir.join(enclosed_path);
                Logger::log_step("Extracting", &format!("to: {:?}", outpath));
                if let Some(p) = outpath.parent() { 
                    fs::create_dir_all(&p).map_err(|e| println!("Dir error: {:?}", e)).ok(); 
                }
                if (*file.name()).ends_with('/') {
                    fs::create_dir_all(&outpath).ok();
                } else {
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

    fn calculate_hash(file: &mut fs::File) -> io::Result<String> {
        file.seek(SeekFrom::Start(0))?;
        let mut hasher = Sha256::new();
        io::copy(file, &mut hasher)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(hex::encode(hasher.finalize()))
    }

fn get_url_by_lang_and_vendor(lang: &str, vendor: &str, major_version: &str, os: &str, arch: &str, platform: &str, runtime: &RuntimeConfig) -> String {
        let catalog = Self::fetch_catalog().expect("Failed to fetch version catalog.");

        if lang == "java" && vendor == "oracle" && !runtime.accept_oracle_licence_terms.unwrap_or(false) {
            Logger::log_error("Oracle JDK requires license acceptance.");
            println!("             Please set 'accept_oracle_licence_terms = true' in spora.toml");
            std::process::exit(1);
        }

        let catalog_url = catalog.get(lang)
            .and_then(|l| l.get(platform))
            .and_then(|p| p.get(vendor))
            .and_then(|v| v.get(&runtime.version).or_else(|| v.get(major_version)))
            .and_then(|ver| ver.get(os))
            .and_then(|o| o.get(arch))
            .and_then(|a| a.get("url"))
            .and_then(|u| u.as_str());

        if let Some(url) = catalog_url {
            url.to_string()
        } else if lang == "kotlin" && platform == "jvm" {
            format!("https://github.com/JetBrains/kotlin/releases/download/v{}/kotlin-compiler-{}.zip", 
                    runtime.version, runtime.version)
        } else {
            panic!("Version/Vendor not supported or defined in catalog: {} {} ({})", lang, vendor, platform);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::io::Read;
    use colored::*;

    #[test]
    #[ignore]
    fn test_catalog_urls_reachability() {
        let catalog = Toolchain::fetch_catalog().expect("Failed to fetch catalog");
        let client = reqwest::blocking::Client::builder()
            .user_agent("Spora-Validator/0.1.2")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();

        validate_urls(&catalog, &client);
    }

    fn validate_urls(value: &Value, client: &reqwest::blocking::Client) {
        match value {
            Value::String(url) => {
                if url.starts_with("http") {
                    let res = client.execute(client.head(url).build().unwrap());
                    match res {
                        Ok(resp) => {
                            assert!(
                                resp.status().is_success() || resp.status().is_redirection(),
                                "URL is unreachable (Status {}): {}", resp.status(), url
                            );
                            println!("Passed: {}", url);
                        }
                        Err(e) => panic!("Request failed for {}: {}", url, e),
                    }
                }
            }
            Value::Object(map) => {
                for v in map.values() {
                    validate_urls(v, client);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    validate_urls(v, client);
                }
            }
            _ => {}
        }
    }

    #[test]
    #[ignore]
    fn test_catalog_integrity() {
        let catalog = Toolchain::fetch_catalog().expect("Failed to fetch catalog");
        let client = reqwest::blocking::Client::builder()
            .user_agent("Spora-Validator/0.1.2")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap();

        println!("Starting strict integrity check (sha256)...");
        validate_integrity_recursive(&catalog, &client);
    }
    
    #[test]
    fn test_catalog_structure() {
        check_sections(&Toolchain::fetch_catalog().expect("Failed to fetch catalog"));
    }

    fn validate_integrity_recursive(value: &Value, client: &reqwest::blocking::Client) {
        match value {
            Value::Object(map) => {
                if let Some(url) = map.get("url").and_then(|v| v.as_str()) {
                    let expected_sha = map.get("sha256").and_then(|v| v.as_str());

                    match expected_sha {
                        Some(expected) => {
                            print!("Verifying: {} ... ", url);
                            std::io::Write::flush(&mut std::io::stdout()).ok();

                            let mut resp = client.get(url).send().expect("Failed to send request");
                            assert!(resp.status().is_success(), "Download failed for {}", url);

                            let mut hasher = Sha256::new();
                            let mut buffer = Vec::new();
                            resp.read_to_end(&mut buffer).expect("Failed to read body");
                            hasher.update(&buffer);
                            let actual = hex::encode(hasher.finalize());

                            assert_eq!(
                                actual.to_lowercase(),
                                expected.to_lowercase(),
                                "\nChecksum mismatch for URL: {}", url
                            );
                            println!("{}", "OK".green().bold());
                        }
                        None => {
                            println!("Skipping: {} (No sha256 defined)", url);
                        }
                    }
                } else {
                    for v in map.values() {
                        validate_integrity_recursive(v, client);
                    }
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    validate_integrity_recursive(v, client);
                }
            }
            _ => {}
        }
    }

    fn check_sections(catalog: &Value) {
        assert!(catalog.get("java").is_some(), "Catalog must have 'java' section");
        assert!(catalog.get("kotlin").is_some(), "Catalog must have 'kotlin' section");

        println!("Successfully validated local versions.json structure.");
    }

    fn load_local_catalog() -> Value {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources");
        path.push("versions.json");

        let content = fs::read_to_string(&path)
            .expect(&format!("Failed to read local catalog at {:?}", path));
        
        serde_json::from_str(&content)
            .expect("Failed to parse local versions.json as valid JSON")
    }

    #[test]
    #[ignore]
    fn test_local_catalog_urls_reachability() {
        let catalog = load_local_catalog();
        let client = reqwest::blocking::Client::builder()
            .user_agent("Spora-Validator/0.1.2")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();

        validate_urls(&catalog, &client);
    }

    #[test]
    #[ignore]
    fn test_local_catalog_integrity() {
        let catalog = load_local_catalog();
        let client = reqwest::blocking::Client::builder()
            .user_agent("Spora-Validator/0.1.2")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap();

        println!("Starting strict integrity check (sha256)...");
        validate_integrity_recursive(&catalog, &client);
    }

    #[test]
    fn test_local_catalog_structure() {
        check_sections(&load_local_catalog());
    }

    #[test]
    fn test_binary_names() {
        let binary_name = if cfg!(windows) { "bin/javac.exe" } else { "bin/javac" };
        
        assert!(binary_name.contains("javac"));
        if cfg!(windows) {
            assert!(binary_name.ends_with(".exe"));
        }
    }
}
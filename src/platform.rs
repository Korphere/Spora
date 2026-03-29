pub struct Platform {
    pub os: &'static str,   // "windows", "linux", "macos"
    pub arch: &'static str, // "x64", "aarch64", "s390x", etc.
}

impl Platform {
    pub fn current() -> Self {
        let os = if cfg!(target_os = "windows") { "windows" }
            else if cfg!(target_os = "macos") { "macos" }
            else if cfg!(target_os = "aix") { "aix" }
            else {
                if !Self::is_musl() {
                    "linux"
                } else {
                    "linux-musl"
                }
            };

        let arch = if cfg!(target_arch = "x86_64") { "x64" }
            else if cfg!(target_arch = "aarch64") { "aarch64" }
            else if cfg!(target_arch = "s390x") { "s390x" }
            else if cfg!(target_arch = "powerpc64") { "ppc64le" } 
            else if cfg!(target_arch = "riscv64") { "riscv64" }
            else if cfg!(target_arch = "arm") { "arm" }
            else { "unknown" };

        Self { os, arch }
    }

    fn is_musl() -> bool {
        std::path::Path::new("/lib/ld-musl-x86_64.so.1").exists()
    }
}
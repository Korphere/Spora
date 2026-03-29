pub struct Platform {
    pub os: &'static str,   // "windows", "linux", "mac"
    pub arch: &'static str, // "x64", "aarch64", "s390x", etc.
}

impl Platform {
    pub fn current() -> Self {
        let os = if cfg!(target_os = "windows") { "windows" }
            else if cfg!(target_os = "macos") { "macos" }
            else { "linux" };

        let arch = if cfg!(target_arch = "x86_64") { "x64" }
            else if cfg!(target_arch = "aarch64") { "aarch64" }
            else if cfg!(target_arch = "s390x") { "s390x" }
            else if cfg!(target_arch = "powerpc64") { "ppc64le" } 
            else if cfg!(target_arch = "riscv64") { "riscv64" }
            else { "unknown" };

        Self { os, arch }
    }
}
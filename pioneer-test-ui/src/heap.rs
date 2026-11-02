#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod glibc {
    const ARENA_MAX: libc::c_int = 4;

    /// glibc gives every thread that mallocs a private arena (up to 8 per core) and keeps the memory freed in it, so
    /// the analysis' short-lived worker threads leave gigabytes resident. A few shared arenas reuse that memory.
    pub fn limit_arenas() {
        if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
            unsafe { libc::mallopt(libc::M_ARENA_MAX, ARENA_MAX) };
        }
    }

    pub fn trim() {
        unsafe { libc::malloc_trim(0) };
    }
}

pub fn limit_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    glibc::limit_arenas();
}

pub fn trim_after_analysis() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    glibc::trim();
}

//! La memoria del proceso, de `/proc/self/status` (solo Linux).

/// Memoria residente ahora y el pico, en KiB.
#[derive(Debug, Clone, Copy, Default)]
pub struct Memory {
    pub rss: u64,
    pub peak: u64,
}

#[must_use]
pub fn now() -> Memory {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return Memory::default();
    };
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap_or(0)
    };
    Memory {
        rss: field("VmRSS:"),
        peak: field("VmHWM:"),
    }
}

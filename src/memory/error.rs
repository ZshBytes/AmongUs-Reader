#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("process not found: {0}")]
    ProcessNotFound(String),
    #[error("module not loaded: {0}")]
    ModuleNotFound(String),
    #[error("failed to open process: {0}")]
    OpenProcessFailed(String),
    #[error("read failed at 0x{address:X}: {reason}")]
    ReadFailed { address: u64, reason: String },
    #[error("invalid pointer: 0x{0:X}")]
    InvalidPointer(u64),
    #[error("invalid utf-16 string")]
    InvalidString,
    #[error("configuration incomplete: {0}")]
    ConfigIncomplete(String),
}

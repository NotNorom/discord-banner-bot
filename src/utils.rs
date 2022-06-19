use std::time::SystemTime;

/// Returns the amount of seconds since UNIX 0.
pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

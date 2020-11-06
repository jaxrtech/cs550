use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn unix_now() -> Duration {
    let now = SystemTime::now();
    let unix_timestamp = now
        .duration_since(UNIX_EPOCH)
        .expect("System time went backwards");

    unix_timestamp
}

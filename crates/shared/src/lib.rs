use std::time::Duration;

use bytes::Bytes;

mod message;
mod request;
mod respond;

pub use message::*;
pub use request::*;
pub use respond::*;

fn default_timeout() -> Duration {
    Duration::from_secs(15)
}

/// # Safety
/// Ensure coverage of unit tests
pub unsafe fn as_bytes<S: serde::Serialize>(s: &S) -> Bytes {
    use bytes::BufMut;
    let mut writer = bytes::BytesMut::new().writer();

    serde_json::to_writer(&mut writer, s).unwrap_unchecked();
    writer.into_inner().freeze()
}

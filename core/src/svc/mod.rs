#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
mod unix;

#[cfg(windows)]
pub use self::windows::*;
#[cfg(not(windows))]
pub use self::unix::*;
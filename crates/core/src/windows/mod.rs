pub mod choco;
pub mod eventlog;
pub mod svc;
pub mod syscall;
pub mod task;
#[allow(clippy::upper_case_acronyms)]
pub mod wmi;
pub mod wua;

#[cfg(windows)]
#[must_use]
pub fn is_root() -> bool {
    use std::{ffi::c_void, mem};

    use windows::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    let mut token = INVALID_HANDLE_VALUE;
    let mut elevated = false;
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() {
            let mut elevation: TOKEN_ELEVATION = mem::zeroed();
            let mut size = mem::size_of::<TOKEN_ELEVATION>() as u32;
            if GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut TOKEN_ELEVATION as *mut c_void),
                size,
                &mut size,
            )
            .is_ok()
            {
                elevated = elevation.TokenIsElevated != 0;
            }

            if token != INVALID_HANDLE_VALUE {
                _ = CloseHandle(token);
            }
        };
    }
    elevated
}

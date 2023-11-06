pub mod choco;
pub mod eventlog;
pub mod svc;
pub mod syscall;
pub mod task;
#[allow(clippy::upper_case_acronyms)]
pub mod wmi;
pub mod wua;

use windows_service::service::{ServiceFailureActions, ServiceInfo};

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

pub struct ServiceInstaller {
    pub info: ServiceInfo,
    pub on_failure: Option<ServiceFailureActions>,
}

impl ServiceInstaller {
    pub fn install_service(&mut self) -> Result<(), crate::Error> {
        let Self { info, on_failure } = self;
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let m =
            ServiceManager::remote_computer("", Option::<&str>::None, ServiceManagerAccess::all())?;

        if m.open_service(&info.name, ServiceAccess::QUERY_STATUS)
            .is_ok()
        {
            tracing::info!("service {:?} already exists", info.name);
            return Ok(());
        }

        let s = m.create_service(info, ServiceAccess::all())?;

        if let Some(action) = on_failure.take() {
            s.update_failure_actions(action)?;
        }

        // TODO:

        // err = eventlog.InstallAsEventCreate(ws.Name, eventlog.Error|eventlog.Warning|eventlog.Info)
        // if err != nil {
        // 	if !strings.Contains(err.Error(), "exists") {
        // 		s.Delete()
        // 		return fmt.Errorf("SetupEventLogSource() failed: %s", err)
        // 	}
        // }

        Ok(())
    }
}

pub struct ServiceUninstaller {
    pub name: String,
}

impl ServiceUninstaller {
    pub fn uninstall_service(&self) -> Result<(), crate::Error> {
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let m =
            ServiceManager::remote_computer("", Option::<&str>::None, ServiceManagerAccess::all())?;

        let s = m.open_service(&self.name, ServiceAccess::DELETE)?;

        s.delete()?;

        Ok(())
    }
}

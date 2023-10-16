use crate::{error::Error, WindowsService};
use tracing::error;
use windows::{
    core::{HSTRING, PCWSTR, PWSTR},
    Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, WIN32_ERROR},
        Security::SC_HANDLE,
        System::Services::{
            CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW,
            QueryServiceConfig2W, QueryServiceConfigW, QueryServiceStatusEx,
            ENUM_SERVICE_STATUS_PROCESSW, QUERY_SERVICE_CONFIGW, SC_ENUM_PROCESS_INFO,
            SC_MANAGER_ALL_ACCESS, SC_STATUS_PROCESS_INFO, SERVICE_ALL_ACCESS, SERVICE_CONFIG,
            SERVICE_CONFIG_DELAYED_AUTO_START_INFO, SERVICE_CONFIG_DESCRIPTION,
            SERVICE_CONFIG_SERVICE_SID_INFO, SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DESCRIPTIONW,
            SERVICE_STATE_ALL, SERVICE_STATUS_PROCESS, SERVICE_WIN32,
        },
    },
};

struct Mgr {
    handle: SC_HANDLE,
}

struct Service {
    name: PCWSTR,
    handle: SC_HANDLE,
}

#[derive(Debug, Default)]
struct Status {
    state: u32,
    accepts: u32,
    /// used to report progress during a lengthy operation
    check_point: u32,
    /// estimated time required for a pending operation, in milliseconds
    wait_hint: u32,
    /// if the service is running, the process identifier of it, and otherwise zero
    process_id: u32,
    /// set if the service has exited with a win32 exit code
    win32_exit_code: u32,
    /// set if the service has exited with a service-specific exit code
    service_specific_exit_code: u32,
}

#[derive(Debug)]
struct Config {
    service_type: u32,
    start_type: u32,
    error_control: u32,
    /// fully qualified path to the service binary file, can also include arguments for an auto-start service
    binary_path_name: PWSTR,
    load_order_group: PWSTR,
    tag_id: u32,
    dependencies: PWSTR,
    /// name of the account under which the service should run
    service_start_name: PWSTR,
    display_name: PWSTR,
    password: PWSTR,
    description: PWSTR,
    /// one of SERVICE_SID_TYPE, the type of sid to use for the service
    sid_type: u32,
    /// the service is started after other auto-start services are started plus a short delay
    delayed_auto_start: bool,
}

impl Service {
    fn query_service_config2(&self, info_level: SERVICE_CONFIG) -> Result<Vec<u8>, Error> {
        let mut n = 1024;
        let mut b = Vec::new();
        loop {
            match unsafe { QueryServiceConfig2W(self.handle, info_level, Some(&mut b), &mut n) } {
                Ok(_) => return Ok(b),
                Err(e)
                    if WIN32_ERROR::from_error(&e)
                        .filter(|code| ERROR_INSUFFICIENT_BUFFER != *code)
                        .is_some()
                        || n <= b.len() as u32 =>
                {
                    return Err(Error::WindowsError(e));
                }
                Err(_) => {
                    b.resize(n as usize, 0);
                }
            }
        }
    }
    fn config(&self) -> Result<Config, Error> {
        let mut p = QUERY_SERVICE_CONFIGW::default();
        let mut n = 1024;
        loop {
            if let Err(e) = unsafe { QueryServiceConfigW(self.handle, Some(&mut p), n, &mut n) } {
                if WIN32_ERROR::from_error(&e)
                    .filter(|code| ERROR_INSUFFICIENT_BUFFER != *code)
                    .is_some()
                    || n <= std::mem::size_of::<QUERY_SERVICE_CONFIGW>() as u32
                {
                    return Err(Error::WindowsError(e));
                }
                continue;
            }
            break;
        }

        let mut b = self.query_service_config2(SERVICE_CONFIG_DESCRIPTION)?;
        let p2 = unsafe { *(b.as_mut_ptr() as *mut SERVICE_DESCRIPTIONW) };
        let mut b = self.query_service_config2(SERVICE_CONFIG_DELAYED_AUTO_START_INFO)?;
        let p3 = unsafe { *(b.as_mut_ptr() as *mut SERVICE_DELAYED_AUTO_START_INFO) };
        let delayed_start = p3.fDelayedAutostart;

        let mut b = self.query_service_config2(SERVICE_CONFIG_SERVICE_SID_INFO)?;
        let sid_type = unsafe { *(b.as_mut_ptr() as *mut u32) };

        Ok(Config {
            service_type: p.dwServiceType.0,
            start_type: p.dwStartType.0,
            error_control: p.dwErrorControl.0,
            binary_path_name: p.lpBinaryPathName,
            load_order_group: p.lpLoadOrderGroup,
            tag_id: p.dwTagId,
            dependencies: p.lpDependencies,
            service_start_name: p.lpServiceStartName,
            display_name: p.lpDisplayName,
            password: PWSTR::null(),
            description: p2.lpDescription,
            sid_type,
            delayed_auto_start: delayed_start.as_bool(),
        })
    }
    fn query(&self) -> Result<Status, Error> {
        let mut needed = 0;
        let mut t = Vec::new();
        unsafe {
            QueryServiceStatusEx(
                self.handle,
                SC_STATUS_PROCESS_INFO,
                Some(&mut t),
                &mut needed,
            )
        }?;

        let statu = unsafe { *(t.as_mut_ptr() as *mut SERVICE_STATUS_PROCESS) };

        Ok(Status {
            state: statu.dwCurrentState.0,
            accepts: statu.dwControlsAccepted,
            process_id: statu.dwProcessId,
            win32_exit_code: statu.dwWin32ExitCode,
            service_specific_exit_code: statu.dwServiceSpecificExitCode,
            // TODO: 這兩個在原先的query裏是沒有設置的
            check_point: statu.dwCheckPoint,
            wait_hint: statu.dwWaitHint,
        })
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        if let Err(e) = unsafe { CloseServiceHandle(self.handle) } {
            error!("close service failed: {e:?}");
        }
    }
}

impl Mgr {
    fn connect() -> Result<Self, Error> {
        let handle = unsafe { OpenSCManagerW(None, None, SC_MANAGER_ALL_ACCESS) }?;
        Ok(Mgr { handle })
    }

    fn open_service(&self, name: PCWSTR) -> Result<Service, Error> {
        let handle = unsafe { OpenServiceW(self.handle, name, SERVICE_ALL_ACCESS) }?;
        Ok(Service { name, handle })
    }

    fn list_services(&self) -> Result<Vec<PCWSTR>, Error> {
        let mut p = Vec::new();
        let mut bytes_needed = 0;
        let mut services_returned = 0;
        loop {
            match unsafe {
                EnumServicesStatusExW(
                    self.handle,
                    SC_ENUM_PROCESS_INFO,
                    SERVICE_WIN32,
                    SERVICE_STATE_ALL,
                    Some(&mut p),
                    &mut bytes_needed,
                    &mut services_returned,
                    None,
                    None,
                )
            } {
                Ok(_) => {
                    break;
                }
                Err(e)
                    if WIN32_ERROR::from_error(&e)
                        .filter(|code| ERROR_MORE_DATA == *code)
                        .is_some()
                        || bytes_needed <= p.len() as u32 =>
                {
                    return Err(Error::WindowsError(e));
                }
                Err(_) => {
                    p.resize(bytes_needed as usize, 0);
                }
            }
        }

        if services_returned == 0 {
            return Ok(vec![]);
        }

        let services = unsafe {
            from_raw_parts_mut::<ENUM_SERVICE_STATUS_PROCESSW>(&mut p, services_returned)
        };

        Ok(services
            .iter_mut()
            .map(|s| PCWSTR::from_raw(s.lpServiceName.as_ptr()))
            .collect())
    }
}

unsafe fn from_raw_parts_mut<T>(p: &mut [u8], services_returned: u32) -> &mut [T] {
    let ptr = p.as_mut_ptr() as *mut T;
    let len = services_returned as usize;
    std::slice::from_raw_parts_mut(ptr, len)
}

impl Drop for Mgr {
    fn drop(&mut self) {
        if let Err(e) = unsafe { CloseServiceHandle(self.handle) } {
            error!("close windows service manager failed: {e:?}");
        }
    }
}

fn get_service_status(name: impl Into<HSTRING>) -> Result<String, Error> {
    let conn = Mgr::connect()?;
    let name: HSTRING = name.into();

    let srv = conn.open_service(PCWSTR::from_raw(name.as_ptr()))?;

    let q = srv.query()?;

    Ok(service_status_text(q.state))
}

fn service_status_text(num: u32) -> String {
    match num {
        1 => "stopped",
        2 => "start_pending",
        3 => "stop_pending",
        4 => "running",
        5 => "continue_pending",
        6 => "pause_pending",
        7 => "paused",
        _ => "unknown",
    }
    .into()
}
fn service_start_type(num: u32) -> String {
    match num {
        0 => "Boot",
        1 => "System",
        2 => "Automatic",
        3 => "Manual",
        4 => "Disabled",
        5 => "Unknown",
        _ => "unknown",
    }
    .into()
}

pub fn get_service() -> Result<Vec<WindowsService>, Error> {
    let conn = Mgr::connect()?;
    let svcs = conn.list_services()?;
    let res = svcs
        .into_iter()
        .filter_map(|s| {
            let handle = || -> Result<WindowsService, Error> {
                let srv = conn.open_service(s)?;
                let q = srv.query()?;
                let config = srv.config()?;
                Ok(unsafe {
                    WindowsService {
                        name: s.to_string()?,
                        status: service_status_text(q.state),
                        display_name: config.display_name.to_string()?,
                        bin_path: config.binary_path_name.to_string()?,
                        description: config.description.to_string()?,
                        username: config.service_start_name.to_string()?,
                        pid: q.process_id,
                        start_type: service_start_type(config.start_type),
                        delayed_auto_start: config.delayed_auto_start,
                    }
                })
            };
            match handle() {
                Ok(res) => Some(res),
                Err(e) => {
                    error!("get service error: {e:?}");
                    None
                }
            }
        })
        .collect();
    Ok(res)
}

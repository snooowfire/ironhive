use std::mem;

use crate::{error::Error, WindowsService};
use tracing::{debug, error};
use windows::{
    core::{HSTRING, PCWSTR, PWSTR},
    Win32::{
        Security::SC_HANDLE,
        System::Services::{
            CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW,
            QueryServiceConfigW, QueryServiceStatusEx, ENUM_SERVICE_STATUS_PROCESSW,
            QUERY_SERVICE_CONFIGW, SC_ENUM_PROCESS_INFO, SC_MANAGER_ALL_ACCESS,
            SC_STATUS_PROCESS_INFO, SERVICE_ALL_ACCESS, SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
            SERVICE_CONFIG_DESCRIPTION, SERVICE_CONFIG_SERVICE_SID_INFO,
            SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DESCRIPTIONW, SERVICE_STATE_ALL,
            SERVICE_STATUS_PROCESS, SERVICE_WIN32,
        },
    },
};

#[derive(Debug)]
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

macro_rules! query_service_config2 {
    ($($name:ident + $info_level:ident => $tp: ty),*) => {
        $(
            fn $name(&self) -> Result<$tp,Error> {
                let mut bytes_needed = 0;
                _ = unsafe { windows::Win32::System::Services::QueryServiceConfig2W(self.handle, $info_level, None, &mut bytes_needed) };
                let mut raw_config = vec![0;bytes_needed as usize];
                unsafe { windows::Win32::System::Services::QueryServiceConfig2W(self.handle, $info_level, Some(&mut raw_config[..]), &mut bytes_needed) }?;
                let config = unsafe { *(raw_config.as_mut_ptr() as *mut $tp) };
                Ok(config)
            }
        )*
    };
}

impl Service {
    query_service_config2! {
        query_delayed_auto_start_info + SERVICE_CONFIG_DELAYED_AUTO_START_INFO => SERVICE_DELAYED_AUTO_START_INFO,
        query_sid_type + SERVICE_CONFIG_SERVICE_SID_INFO => u32,
        query_description + SERVICE_CONFIG_DESCRIPTION => SERVICE_DESCRIPTIONW
    }

    fn config(&self) -> Result<Config, Error> {
        let mut bytes_needed = 0;

        _ = unsafe { QueryServiceConfigW(self.handle, None, 0, &mut bytes_needed) };

        let mut raw_config = unsafe { mem::zeroed::<QUERY_SERVICE_CONFIGW>() };

        unsafe {
            QueryServiceConfigW(
                self.handle,
                Some(&mut raw_config),
                bytes_needed,
                &mut bytes_needed,
            )
        }?;

        let description = self.query_description()?;

        let delayed_auto_start_info = self.query_delayed_auto_start_info()?;

        let sid_type = self.query_sid_type()?;

        let config = Config {
            service_type: raw_config.dwServiceType.0,
            start_type: raw_config.dwStartType.0,
            error_control: raw_config.dwErrorControl.0,
            binary_path_name: raw_config.lpBinaryPathName,
            load_order_group: raw_config.lpLoadOrderGroup,
            tag_id: raw_config.dwTagId,
            dependencies: raw_config.lpDependencies,
            service_start_name: raw_config.lpServiceStartName,
            display_name: raw_config.lpDisplayName,
            password: PWSTR::null(),
            description: description.lpDescription,
            sid_type,
            delayed_auto_start: delayed_auto_start_info.fDelayedAutostart.as_bool(),
        };

        debug!("query config ok: {config:?}");

        Ok(config)
    }

    fn query(&self) -> Result<Status, Error> {
        let mut bytes_needed = 0;
        _ = unsafe {
            QueryServiceStatusEx(self.handle, SC_STATUS_PROCESS_INFO, None, &mut bytes_needed)
        };

        let mut buffer = vec![0; bytes_needed as usize];

        unsafe {
            QueryServiceStatusEx(
                self.handle,
                SC_STATUS_PROCESS_INFO,
                Some(&mut buffer),
                &mut bytes_needed,
            )
        }?;

        let statu = unsafe { *(buffer.as_mut_ptr() as *mut SERVICE_STATUS_PROCESS) };

        Ok(Status {
            state: statu.dwCurrentState.0,
            accepts: statu.dwControlsAccepted,
            process_id: statu.dwProcessId,
            win32_exit_code: statu.dwWin32ExitCode,
            service_specific_exit_code: statu.dwServiceSpecificExitCode,
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
        let handle = unsafe { OpenSCManagerW(PCWSTR::null(), None, SC_MANAGER_ALL_ACCESS) }?;
        debug!("connect well");
        Ok(Mgr { handle })
    }

    fn open_service(&self, name: PCWSTR) -> Result<Service, Error> {
        debug!("start open_service {name:?}");

        let handle = unsafe { OpenServiceW(self.handle, name, SERVICE_ALL_ACCESS) }?;

        debug!("open_service well");
        Ok(Service { name, handle })
    }

    fn list_services(&self) -> Result<Vec<PCWSTR>, Error> {
        let mut bytes_needed = unsafe { mem::zeroed::<u32>() };
        let mut services_returned = unsafe { mem::zeroed::<u32>() };

        _ = unsafe {
            EnumServicesStatusExW(
                self.handle,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                None,
                &mut bytes_needed,
                &mut services_returned,
                None,
                None,
            )
        };

        let mut raw_services = vec![0; bytes_needed as usize];

        unsafe {
            EnumServicesStatusExW(
                self.handle,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                Some(&mut raw_services),
                &mut bytes_needed,
                &mut services_returned,
                None,
                None,
            )
        }?;

        if services_returned == 0 {
            return Ok(vec![]);
        }

        let ptr = raw_services.as_mut_ptr() as *mut ENUM_SERVICE_STATUS_PROCESSW;
        let len = services_returned as usize;

        let services = unsafe { std::slice::from_raw_parts_mut(ptr, len) };

        debug!("list_services well");

        Ok(services
            .iter_mut()
            .map(|s| PCWSTR::from_raw(s.lpServiceName.as_ptr()))
            .collect())
    }
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
    debug!("start get_service");
    let conn = Mgr::connect()?;
    debug!("create conn fine.");
    let svcs = conn.list_services()?;
    debug!("list services fine: {svcs:?}");
    let res = svcs
        .into_iter()
        .filter_map(|s| get_config(&conn, s).ok())
        .collect();
    debug!("get_service well..");
    Ok(res)
}

fn get_config(conn: &Mgr, service: PCWSTR) -> Result<WindowsService, Error> {
    debug!("will get {service:?} config");
    let srv = conn.open_service(service)?;
    let q = srv.query()?;
    let config = srv.config()?;
    debug!("get config {config:?}");
    debug!("{} live!", unsafe { service.display() });
    Ok(unsafe {
        WindowsService {
            name: service.to_string()?,
            status: service_status_text(q.state),
            display_name: config.display_name.to_string()?,
            bin_path: config.binary_path_name.to_string()?,
            // TODO: 奇怪的bug，调用to_string就崩溃
            // description: config.description.to_string()?,
            username: config.service_start_name.to_string()?,
            pid: q.process_id,
            start_type: service_start_type(config.start_type),
            delayed_auto_start: config.delayed_auto_start,
            ..Default::default()
        }
    })
}

#[test]
#[tracing_test::traced_test]
fn test_get_config() {
    let conn = Mgr::connect().unwrap();
    let svrs = conn.list_services().unwrap();
    for svc in svrs {
        // let wide = unsafe { svc.to_string() };
        // println!("will handle {:?}", wide);

        let config = get_config(&conn, svc);
        println!("config: {config:?}");
    }
}

#[test]
#[tracing_test::traced_test]
fn test_get_service() {
    let service = get_service();
    assert!(service.is_ok());
}

#[test]
fn test_mgr() {
    let mgr = Mgr::connect();
    println!("{mgr:?}");
}

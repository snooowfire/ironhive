use std::{mem, time::Duration};

use crate::error::Error;
use chrono::NaiveDate;
use shared::{WinSoftwareList, WindowsService};
use std::fmt::Debug;
use tracing::debug;
use windows::{
    core::{IntoParam, PCWSTR},
    Win32::{
        Foundation::{
            ERROR_INVALID_SERVICE_CONTROL, ERROR_SERVICE_CANNOT_ACCEPT_CTRL,
            ERROR_SERVICE_NOT_ACTIVE,
        },
        Security::SC_HANDLE,
        System::Services::{
            ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService,
            EnumServicesStatusExW, OpenSCManagerW, OpenServiceW, QueryServiceConfigW,
            QueryServiceStatusEx, StartServiceW, ENUM_SERVICE_STATUS_PROCESSW,
            QUERY_SERVICE_CONFIGW, SC_ENUM_PROCESS_INFO, SC_MANAGER_ALL_ACCESS,
            SC_STATUS_PROCESS_INFO, SERVICE_ALL_ACCESS, SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
            SERVICE_CONFIG_DESCRIPTION, SERVICE_CONFIG_SERVICE_SID_INFO, SERVICE_CONTROL_STOP,
            SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DESCRIPTIONW, SERVICE_ERROR,
            SERVICE_START_TYPE, SERVICE_STATE_ALL, SERVICE_STATUS, SERVICE_STATUS_PROCESS,
            SERVICE_STOPPED, SERVICE_WIN32,
        },
    },
};
use winreg::enums::{KEY_ENUMERATE_SUB_KEYS, KEY_QUERY_VALUE};

#[derive(Debug)]
struct Mgr {
    handle: SC_HANDLE,
}

struct Service {
    name: String,
    handle: SC_HANDLE,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug)]
struct Config {
    service_type: u32,
    start_type: u32,
    error_control: u32,
    /// fully qualified path to the service binary file, can also include arguments for an auto-start service
    binary_path_name: String,
    load_order_group: String,
    tag_id: u32,
    dependencies: String,
    /// name of the account under which the service should run
    service_start_name: String,
    display_name: String,
    /// TODO: Password is not returned by windows.QueryServiceConfig, not sure how to get it.
    /// https://cs.opensource.google/go/x/sys/+/master:windows/svc/mgr/config.go;drc=1bfbee0e20e3039533666df89a91c1876e67605d;l=30
    password: String,
    description: String,
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
                let config = unsafe { (raw_config.as_mut_ptr() as *mut $tp).read() };
                Ok(config)
            }
        )*
    };
}

macro_rules! to_string {
    ($wstr: expr) => {
        if $wstr.is_null() {
            Ok(String::default())
        } else {
            unsafe { $wstr.to_string() }
        }
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
            binary_path_name: to_string!(raw_config.lpBinaryPathName)?,
            load_order_group: to_string!(raw_config.lpLoadOrderGroup)?,
            tag_id: raw_config.dwTagId,
            dependencies: to_string!(raw_config.lpDependencies)?,
            service_start_name: to_string!(raw_config.lpServiceStartName)?,
            display_name: to_string!(raw_config.lpDisplayName)?,
            password: Default::default(),
            description: to_string!(description.lpDescription)?,
            sid_type,
            delayed_auto_start: delayed_auto_start_info.fDelayedAutostart.as_bool(),
        };

        debug!("query config ok: {config:?}");

        Ok(config)
    }

    fn update_config(&self, config: Config) -> Result<(), Error> {
        unsafe {
            ChangeServiceConfigW(
                self.handle,
                config.service_type,
                SERVICE_START_TYPE(config.start_type),
                SERVICE_ERROR(config.error_control),
                &config.binary_path_name.into(),
                &config.load_order_group.into(),
                None,
                &config.dependencies.into(),
                &config.service_start_name.into(),
                &config.password.into(),
                &config.display_name.into(),
            )
        }?;

        unsafe {
            ChangeServiceConfig2W(
                self.handle,
                SERVICE_CONFIG_SERVICE_SID_INFO,
                Some(&config.sid_type as *const u32 as _),
            )
        }?;

        let start_info = SERVICE_DELAYED_AUTO_START_INFO {
            fDelayedAutostart: config.delayed_auto_start.into(),
        };

        unsafe {
            ChangeServiceConfig2W(
                self.handle,
                SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                Some(&start_info as *const SERVICE_DELAYED_AUTO_START_INFO as _),
            )
        }?;

        Ok(())
    }

    fn control(&self, cmd: u32) -> Result<Status, Error> {
        let mut status = SERVICE_STATUS::default();
        if let Err(e) = unsafe { ControlService(self.handle, cmd, &mut status) } {
            if e != ERROR_INVALID_SERVICE_CONTROL.into()
                && e != ERROR_SERVICE_CANNOT_ACCEPT_CTRL.into()
                && e != ERROR_SERVICE_NOT_ACTIVE.into()
            {
                return Err(Error::WindowsError(e));
            }
        }

        Ok(Status {
            state: status.dwCurrentState.0,
            accepts: status.dwControlsAccepted,
            ..Default::default()
        })
    }

    fn start(&self) -> Result<(), Error> {
        unsafe { StartServiceW(self.handle, None) }?;
        Ok(())
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

        let statu = unsafe { (buffer.as_mut_ptr() as *mut SERVICE_STATUS_PROCESS).read() };

        Ok(Status {
            state: statu.dwCurrentState.0,
            accepts: statu.dwControlsAccepted,
            process_id: statu.dwProcessId,
            win32_exit_code: statu.dwWin32ExitCode,
            service_specific_exit_code: statu.dwServiceSpecificExitCode,
            ..Default::default()
        })
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        _ = unsafe { CloseServiceHandle(self.handle) };
    }
}

impl Mgr {
    fn connect() -> Result<Self, Error> {
        let handle = unsafe { OpenSCManagerW(PCWSTR::null(), None, SC_MANAGER_ALL_ACCESS) }?;
        debug!("connect well");
        Ok(Mgr { handle })
    }

    fn open_service<N: IntoParam<PCWSTR> + Copy>(&self, name: N) -> Result<Service, Error> {
        let handle = unsafe { OpenServiceW(self.handle, name, SERVICE_ALL_ACCESS) }?;

        let name = name.into_param().abi();
        Ok(Service {
            name: unsafe { name.to_string() }?,
            handle,
        })
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
        _ = unsafe { CloseServiceHandle(self.handle) };
    }
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

pub fn get_services() -> Result<Vec<WindowsService>, Error> {
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

pub fn get_service_detail(name: String) -> Result<WindowsService, Error> {
    let conn = Mgr::connect()?;

    get_config(&conn, &name.into())
}

pub fn edit_service(name: String, startup_type: String) -> Result<(), Error> {
    let conn = Mgr::connect()?;

    let srv = conn.open_service(&name.into())?;

    let mut conf = srv.config()?;

    let start_type = match startup_type.as_str() {
        "auto" | "autodelay" => 2,
        "manual" => 3,
        "disabled" => 4,
        unknow => {
            let err = windows::core::Error::new(
                windows::core::Error::OK.into(),
                format!("Unknown startup type provided: {unknow}").into(),
            );
            return Err(Error::WindowsError(err));
        }
    };

    conf.start_type = start_type;

    if startup_type.eq("autodelay") {
        conf.delayed_auto_start = true;
    } else if startup_type.eq("auto") {
        conf.delayed_auto_start = false;
    }

    srv.update_config(conf)?;

    Ok(())
}

pub fn control_service(name: String, action: String) -> Result<(), Error> {
    let conn = Mgr::connect()?;

    let srv = conn.open_service(&name.into())?;

    match action.as_str() {
        "stop" => {
            let mut status = srv.control(SERVICE_CONTROL_STOP)?;

            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(30);
            while status.state != SERVICE_STOPPED.0 {
                let now = start.elapsed();

                if now > timeout {
                    let err = windows::core::Error::new(
                        windows::core::Error::OK.into(),
                        "Timed out waiting for service to stop".into(),
                    );
                    return Err(Error::WindowsError(err));
                }

                std::thread::sleep(Duration::from_millis(500));

                status = srv.query()?;
            }
        }
        "start" => {
            srv.start()?;
        }

        unknow => {
            let err = windows::core::Error::new(
                windows::core::Error::OK.into(),
                format!("Unknown service action provided: {unknow}").into(),
            );
            return Err(Error::WindowsError(err));
        }
    }

    Ok(())
}

fn get_config<N: IntoParam<PCWSTR> + Copy>(
    conn: &Mgr,
    service: N,
) -> Result<WindowsService, Error> {
    let srv = conn.open_service(service)?;
    let q = srv.query()?;
    let config = srv.config()?;
    debug!("get config {config:?}");
    debug!("{:?} live!", srv.name);

    Ok(WindowsService {
        name: srv.name.clone(),
        status: service_status_text(q.state),
        display_name: config.display_name,
        bin_path: config.binary_path_name,
        description: config.description,
        username: config.service_start_name,
        pid: q.process_id,
        start_type: service_start_type(config.start_type),
        delayed_auto_start: config.delayed_auto_start,
    })
}

#[cfg(target_arch = "x86_64")]
pub fn installed_software_list() -> Result<Vec<WinSoftwareList>, Error> {
    let mut sw64 =
        get_software_list(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall".into())?;
    let mut sw32 = get_software_list(
        r"SOFTWARE\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall".into(),
    )?;
    sw64.append(&mut sw32);
    Ok(sw64)
}

#[cfg(target_arch = "x86")]
pub fn installed_software_list() -> Result<Vec<WinSoftwareList>, Error> {
    let sw32 = get_software_list(r#"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"#.into())?;

    Ok(sw32)
}

fn get_software_list(basekey: String) -> Result<Vec<WinSoftwareList>, Error> {
    let k = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(basekey.clone(), KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS)?;

    let mut sw_list = Vec::new();

    let subkeys = k.enum_keys();

    for sw in subkeys {
        let sw = sw?;
        let sk = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(basekey.clone() + r"\" + sw.as_str(), KEY_QUERY_VALUE)?;

        if let Ok(dn) = sk.get_value::<String, _>("DisplayName") {
            let mut swv = WinSoftwareList {
                name: dn,
                ..Default::default()
            };

            macro_rules! _software_list {
                ($field: ident => $name:literal) => {
                    if let Ok($field) = sk.get_value::<String, _>($name) {
                        swv.$field = $field;
                    }
                };
                ($field: ident => $name:literal = $target: ty) => {
                    if let Ok($field) = sk.get_value::<$target, _>($name) {
                        swv.$field = $field;
                    }
                };
                ($field: ident in $map: expr => $name:literal) => {
                    if let Ok($field) = sk.get_value::<String, _>($name) {
                        swv.$field = $map;
                    }
                };
                ($field: ident in $map: expr => $name:literal = $target: ty) => {
                    if let Ok($field) = sk.get_value::<$target, _>($name) {
                        swv.$field = $map;
                    }
                };
            }

            macro_rules! software_list {
                ($($field: ident $(in $map: expr)? => $name:literal $(= $target: ty)?),*) => {
                    $(
                        _software_list!($field $(in $map)? => $name $(= $target)?);
                    )*
                };
            }

            software_list! {
                version => "DisplayVersion",
                publisher => "Publisher",
                install_date in NaiveDate::parse_from_str(&install_date, "%Y%m%d").map(|date| date.format("%Y-%m-%d").to_string()).unwrap_or_default() => "InstallDate",
                size in humansize::format_size(size * 1024, humansize::WINDOWS) => "EstimatedSize" = u64,
                source => "InstallSource",
                location => "InstallLocation",
                uninstall => "UninstallString"
            }

            sw_list.push(swv);
        }
    }

    Ok(sw_list)
}

#[test]
fn test_installed_software_list() {
    let res = installed_software_list();

    println!("{res:?}");

    assert!(res.is_ok());
}

#[test]
fn test_description() {
    let conn = Mgr::connect().unwrap();
    let svrs = conn.list_services().unwrap();
    for svc in svrs {
        let wide = to_string!(svc);
        debug!("will handle {:?}", wide);

        if let Ok(svc) = conn.open_service(svc) {
            let description = svc.query_description();
            debug!("{description:?}");
            if let Ok(raw) = description {
                let s = to_string!(raw.lpDescription);

                debug!("{s:?}");
            }
        }
    }
}

#[test]
#[tracing_test::traced_test]
fn test_get_config() {
    let conn = Mgr::connect().unwrap();
    let svrs = conn.list_services().unwrap();
    for svc in svrs {
        let config = get_config(&conn, svc);
        debug!("config: {config:?}");
    }
}

#[test]
#[tracing_test::traced_test]
fn test_get_service() {
    let service = get_services();
    assert!(service.is_ok());
}

#[test]
fn test_mgr() {
    let mgr = Mgr::connect();
    debug!("{mgr:?}");
}

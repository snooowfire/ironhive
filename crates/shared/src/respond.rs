use std::time::Duration;

use crate::message::{ProcessMsg, WUAPackage, WinSoftwareList, WindowsService};

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(serde::Serialize))]
#[cfg_attr(feature = "client", derive(serde::Deserialize))]
#[serde(rename_all = "lowercase")]
#[serde(tag = "resp")]
pub enum IronhiveRespond {
    Pong,
    ProcessMsg {
        msgs: Vec<ProcessMsg>,
    },
    Ok,
    RawCMDResp {
        results: String,
    },
    RunScriptResp {
        stdout: String,
        stderr: String,
        retcode: i32,
        #[serde(with = "humantime_serde")]
        execution_time: Duration,
        id: i32,
    },
    NeedsReboot {
        needs: bool,
    },
    CpuLoadAvg {
        /// Average load within one minute.
        one: f64,
        ///Average load within five minutes.
        five: f64,
        /// Average load within fifteen minutes.
        fifteen: f64,
    },
    CpuUssage {
        usage: f32,
    },
    PublicIp {
        ip: String,
    },
    WinSoftwareNats {
        software: Vec<WinSoftwareList>,
    },

    WinUpdateResult {
        updates: Vec<WUAPackage>,
    },
    WinServices {
        services: Vec<WindowsService>,
    },
    WinSvcDetail {
        service: WindowsService,
    },
    WinSvcResp {
        success: bool,
        errormsg: String,
    },
}

impl IronhiveRespond {
    #[cfg(feature = "server")]
    pub fn as_bytes(&self) -> bytes::Bytes {
        unsafe { crate::as_bytes(self) }
    }
}

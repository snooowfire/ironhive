use std::{collections::HashMap, time::Duration};

use crate::{
    default_timeout,
    message::{AgentMode, ScriptMode},
};

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(serde::Deserialize))]
#[cfg_attr(feature = "client", derive(serde::Serialize))]
#[serde(rename_all = "lowercase")]
#[serde(tag = "func")]
pub enum IronhiveRequest {
    Ping,
    PatchMgmt {
        patch_mgmnt: bool,
    },
    // SchedTask,
    // DelSchedTask,
    // ListSchedTasks,
    // EventLog,
    Procs,
    KillProc {
        proc_pid: u32,
    },
    RawCmd {
        shell: String,
        command: String,
        #[serde(with = "humantime_serde")]
        #[serde(default = "default_timeout")]
        timeout: Duration,
        // run_as_user: bool,
    },
    WinServices,
    WinSvcDetail {
        name: String,
    },
    WinSvcAction {
        name: String,
        action: String,
    },
    EditWinSvc {
        name: String,
        start_type: String,
    },
    RunScript {
        code: String,
        #[serde(default)]
        mode: ScriptMode,
        #[serde(default)]
        script_args: Vec<String>,
        #[serde(with = "humantime_serde")]
        #[serde(default = "default_timeout")]
        timeout: Duration,
        // run_as_user: bool,
        #[serde(default)]
        env_vars: HashMap<String, String>,
        id: i32,
    },
    SoftwareList,
    RebootNow,
    NeedsReboot,
    SysInfo,
    WMI,
    CpuLoadAvg,
    CpuUssage,
    // RunChecks,
    // RunTask,
    PublicIp,
    // InstallPython,
    InstallChoco,
    InstallWithChoco {
        choco_prog_name: String,
    },
    GetWinUpdates,
    InstallWinUpdates {
        update_guids: Vec<String>,
    },
    // AgentUpdate,
    // Uninstall,
    Checkin {
        mode: AgentMode,
    },
}

impl IronhiveRequest {
    #[cfg(feature = "client")]
    pub fn as_bytes(&self) -> bytes::Bytes {
        unsafe { crate::as_bytes(self) }
    }
}

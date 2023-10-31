use std::{collections::HashMap, path::PathBuf, time::Duration};

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct CheckInNats {
    pub agent_id: String,
    pub version: String,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct AgentInfoNats {
    pub agent_id: String,
    #[serde(rename = "logged_in_username")]
    pub username: String,
    pub hostname: String,
    #[serde(rename = "operating_system")]
    pub os: String,
    pub plat: String,
    pub total_ram: u64,
    pub boot_time: u64,
    #[serde(rename = "needs_reboot")]
    pub reboot_needed: bool,
    pub arch: String,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct WinSvcNats {
    pub agent_id: String,
    #[serde(rename = "services")]
    pub win_svcs: Vec<WindowsService>,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct WindowsService {
    pub name: String,
    pub status: String,
    pub display_name: String,
    #[serde(rename = "binpath")]
    pub bin_path: String,
    pub description: String,
    pub username: String,
    pub pid: u32,
    pub start_type: String,
    #[serde(rename = "autodelay")]
    pub delayed_auto_start: bool,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct WinWMINats {
    pub agent_id: String,
    pub wmi: serde_json::Value, // Use serde_json::Value for dynamic deserialization
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct WinDisksNats {
    pub agent_id: String,
    pub disks: Vec<Disk>,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct Disk {
    pub device: String,
    pub fstype: String,
    pub total: String,
    pub used: String,
    pub free: String,
    pub percent: i32,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct PublicIPNats {
    pub agent_id: String,
    pub public_ip: String,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub struct WinSoftwareList {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub install_date: String,
    pub size: String,
    pub source: String,
    pub location: String,
    pub uninstall: String,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(serde::Deserialize))]
pub struct ProcessMsg {
    pub name: String,
    pub pid: u32,
    pub membytes: u64,
    pub username: String,
    pub id: String,
    pub cpu_percent: String,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(serde::Deserialize))]
pub struct RawCMDResp {
    pub results: String,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(serde::Deserialize))]
pub struct RunScriptResp {
    pub stdout: String,
    pub stderr: String,
    pub retcode: i32,
    #[serde(with = "humantime_serde")]
    pub execution_time: Duration,
    pub id: i32,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(serde::Deserialize))]
pub struct WUAPackage {
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub category_ids: Vec<String>,
    pub kb_article_ids: Vec<String>,
    pub more_info_urls: Vec<String>,
    pub support_url: String,
    pub guid: String,
    pub revision_number: i32,
    pub severity: String,
    pub installed: bool,
    pub downloaded: bool,
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(serde::Deserialize))]
pub struct AutomatedTask {
    pub id: i32,
    pub task_actions: Vec<TaskAction>,
    pub enabled: bool,
    pub continue_on_error: bool,
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(serde::Deserialize))]
pub enum TaskAction {
    CmdScript {
        code: String,
        mode: ScriptMode,
        args: Vec<String>,
        env_vars: HashMap<String, String>,
        detached: bool,
        timeout: Duration,
    },
    CmdShell {
        shell: String,
        command: String,
        detached: bool,
        timeout: Duration,
    },
}

#[derive(Debug, PartialEq, Default, Clone)]
#[cfg_attr(
    any(feature = "server", feature = "client"),
    derive(Deserialize, Serialize)
)]
pub enum ScriptMode {
    PowerShell,
    Binary {
        path: PathBuf,
        ext: String,
    },
    Cmd,
    #[default]
    Directly,
}

impl ScriptMode {
    pub fn ext(&self) -> &str {
        match self {
            ScriptMode::PowerShell => ".ps1",
            ScriptMode::Binary { ext, .. } => ext.as_str(),
            ScriptMode::Cmd => ".bat",
            ScriptMode::Directly => "",
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(
    any(feature = "server", feature = "client"),
    derive(Deserialize, Serialize)
)]
pub enum AgentMode {
    #[serde(rename = "agent-hello")]
    Hello,
    #[serde(rename = "agent-winsvc")]
    WinSvc,
    #[serde(rename = "agent-agentinfo")]
    AgentInfo,
    #[serde(rename = "agent-wmi")]
    WMI,
    #[serde(rename = "agent-disks")]
    Disks,
    #[serde(rename = "agent-publicip")]
    PublicIp,
}

impl AgentMode {
    pub fn all() -> [AgentMode; 6] {
        [
            Self::Hello,
            Self::WinSvc,
            Self::AgentInfo,
            Self::WMI,
            Self::Disks,
            Self::PublicIp,
        ]
    }
}

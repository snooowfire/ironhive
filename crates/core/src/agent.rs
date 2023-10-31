use std::{ops::Deref, time::Duration};

use crate::cmd::CmdExe;
use crate::error::Error;
use async_nats::ToServerAddrs;
use shared::ProcessMsg;
use sysinfo::{CpuExt, DiskExt, Pid, PidExt, ProcessExt, SystemExt, UserExt};

// 定义Agent结构体
#[derive(Debug)]
pub struct Agent {
    pub agent_id: String,
    pub nats_servers: Vec<async_nats::ServerAddr>,
    pub system: sysinfo::System,
    version: String,
    host_name: String,
}

impl Default for Agent {
    fn default() -> Self {
        let system = sysinfo::System::new_all();
        Self {
            agent_id: Default::default(),
            version: env!("CARGO_PKG_VERSION").into(),
            host_name: system.host_name().unwrap_or_default(),
            system,
            nats_servers: Default::default(),
        }
    }
}

impl Agent {
    pub fn new<A>(agent_id: String, nats_servers: &A) -> Result<Self, Error>
    where
        A: ToServerAddrs,
    {
        Ok(Self {
            agent_id,
            nats_servers: nats_servers.to_server_addrs()?.collect(),
            ..Default::default()
        })
    }

    pub fn with_servers<S, A>(agent_id: String, nats_servers: A) -> Self
    where
        A: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            agent_id,
            nats_servers: nats_servers
                .into_iter()
                .filter_map(|svr| {
                    let s = svr.as_ref();
                    match s.to_server_addrs() {
                        Ok(addrs) => Some(addrs),
                        Err(e) => {
                            tracing::error!("convert nats server {s} failed: {e:?}");
                            None
                        }
                    }
                })
                .flatten()
                .collect(),
            ..Default::default()
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn host_name(&self) -> &str {
        &self.host_name
    }

    pub fn get_cpu_usage(&self) -> f32 {
        self.system.global_cpu_info().cpu_usage()
    }

    pub fn get_load_avg(&self) -> sysinfo::LoadAvg {
        self.system.load_average()
    }

    pub fn get_disks(&self) -> Vec<shared::Disk> {
        self.system
            .disks()
            .iter()
            .filter(|d| {
                if cfg!(windows) {
                    true
                } else {
                    let name = d.name().to_string_lossy();
                    !(name.contains("dev/loop") || name.contains("devfs"))
                }
            })
            .map(|d| shared::Disk {
                device: d.name().to_string_lossy().to_string(),
                fstype: String::from_utf8_lossy(d.file_system()).to_string(),
                total: humansize::format_size(d.total_space(), humansize::DECIMAL),
                used: humansize::format_size(
                    d.total_space() - d.available_space(),
                    humansize::DECIMAL,
                ),
                free: humansize::format_size(d.available_space(), humansize::DECIMAL),
                percent: ((d.total_space() - d.available_space()) * 100 / d.total_space()) as i32,
            })
            .collect()
    }

    pub fn os_string(&self) -> String {
        format!(
            "{} {} {}",
            self.system.long_os_version().unwrap_or_default(),
            std::env::consts::ARCH,
            self.system.kernel_version().unwrap_or_default()
        )
    }

    pub fn get_procs_rpc(&self) -> Vec<ProcessMsg> {
        self.system
            .processes()
            .iter()
            .filter_map(|(id, p)| {
                if id.as_u32() != 0 {
                    Some(ProcessMsg {
                        name: p.name().into(),
                        pid: id.as_u32(),
                        membytes: p.memory(),
                        username: p
                            .user_id()
                            .and_then(|uid| self.system.get_user_by_id(uid))
                            .map(|user| user.name())
                            .unwrap_or("")
                            .to_string(),
                        id: p
                            .user_id()
                            .map(|uid| format!("{}", uid.deref()))
                            .unwrap_or_default(),
                        cpu_percent: format!("{:.1}%", p.cpu_usage()),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn kill_proc(&self, pid: Pid) -> Result<(), Error> {
        let process: &sysinfo::Process = self
            .system
            .process(pid)
            .ok_or(Error::NotFoundProcess(pid))?;
        if !process.kill() {
            return Err(Error::KillProcessFailed(pid));
        }

        Ok(())
    }
}

pub fn logged_on_user() -> String {
    whoami::username()
}

pub async fn reboot_now() -> Result<(), Error> {
    #[cfg(windows)]
    {
        CmdExe {
            exe: "shutdown.exe",
            args: vec!["/r", "/t", "5", "/f"],
            detached: false,
            timeout: Duration::from_secs(15),
        }
        .run()
        .await?;
    }
    #[cfg(not(windows))]
    {
        CmdExe::<_, &str> {
            exe: "reboot",
            args: vec![],
            detached: false,
            timeout: Duration::from_secs(15),
        }
        .run()
        .await?;
    }

    Ok(())
}

#[cfg(windows)]
pub fn system_reboot_required_win() -> bool {
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    hklm.open_subkey(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired",
    )
    .is_ok()
}

pub async fn system_reboot_required() -> bool {
    #[cfg(windows)]
    {
        system_reboot_required_win()
    }
    #[cfg(not(windows))]
    {
        use std::path::Path;
        use tracing::debug;
        // deb
        if ["/var/run/reboot-required", "/run/reboot-required"]
            .iter()
            .any(|p| Path::new(p).exists())
        {
            return true;
        }
        // // rhel
        for bin in ["/usr/bin/needs-restarting", "/bin/needs-restarting"]
            .iter()
            .filter(|p| Path::new(p).exists())
        {
            let res = CmdExe {
                exe: bin,
                args: vec!["-r"],
                detached: false,
                timeout: Duration::from_secs(15),
            }
            .run()
            .await;

            match res {
                Ok(o) => {
                    if o.status.success() {
                        return true;
                    }
                }
                Err(e) => {
                    debug!("system_reboot_required(): {e:?}");
                }
            }
        }

        false
    }
}

#[allow(unused_variables)]
pub fn patch_mgmnt(enable: bool) -> Result<(), Error> {
    #[cfg(windows)]
    {
        let k = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
        let (key, _) = k.create_subkey_with_flags(
            r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
            winreg::enums::KEY_ALL_ACCESS,
        )?;

        key.set_value("AUOptions", if enable { &1_u64 } else { &0 })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_agent() {
        let agent = Agent {
            agent_id: "agent_id".into(),
            version: "version".into(),
            ..Default::default()
        };
        println!("{}", agent.get_cpu_usage());
        println!("{:?}", agent.get_load_avg());
        println!("{:?}", agent.get_disks());
        println!("{}", logged_on_user());
        println!("{}", agent.os_string());
        println!("{}", system_reboot_required().await);
        println!("{:?}", agent.get_procs_rpc());
    }
}

use std::{ops::Deref, time::Duration};

use crate::cmd::CmdExe;
use crate::error::Error;
use crate::shared::ProcessMsg;
use sysinfo::{CpuExt, DiskExt, Pid, PidExt, ProcessExt, SystemExt, UserExt};

// 定义Agent结构体
#[derive(Debug)]
pub struct Agent {
    pub agent_id: String,
    /// uses recommended semver validation expression from
    /// https://semver.org/#is-there-a-suggested-regular-expression-regex-to-check-a-semver-string
    /// `^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$`
    pub version: String,
    /// From ADR-33: Name can only have A-Z, a-z, 0-9, dash, underscore.
    pub host_name: String,
    pub nats_server: String,
    pub system: sysinfo::System,
}

impl Default for Agent {
    fn default() -> Self {
        Self {
            agent_id: Default::default(),
            version: Default::default(),
            host_name: Default::default(),
            nats_server: Default::default(),
            system: sysinfo::System::new_all(),
        }
    }
}

impl Agent {
    pub async fn reboot_now(&self) -> Result<(), Error> {
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
    pub fn get_cpu_usage(&self) -> f32 {
        self.system.global_cpu_info().cpu_usage()
    }

    pub fn get_load_avg(&self) -> sysinfo::LoadAvg {
        self.system.load_average()
    }

    pub fn get_disks(&self) -> Vec<super::Disk> {
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
            .map(|d| super::Disk {
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

    pub fn logged_on_user(&self) -> String {
        whoami::username()
    }

    pub fn os_string(&self) -> String {
        format!(
            "{} {} {}",
            self.system.long_os_version().unwrap_or_default(),
            std::env::consts::ARCH,
            self.system.kernel_version().unwrap_or_default()
        )
    }

    pub async fn system_reboot_required(&self) -> bool {
        #[cfg(windows)]
        {
            let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
            hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired").is_ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_agent() {
        let agent = Agent {
            agent_id: "agent_id".into(),
            host_name: "host_name".into(),
            nats_server: "nats_server".into(),
            version: "version".into(),
            ..Default::default()
        };
        println!("{}", agent.get_cpu_usage());
        println!("{:?}", agent.get_load_avg());
        println!("{:?}", agent.get_disks());
        println!("{}", agent.logged_on_user());
        println!("{}", agent.os_string());
        println!("{}", agent.system_reboot_required().await);
        println!("{:?}", agent.get_procs_rpc());
    }
}

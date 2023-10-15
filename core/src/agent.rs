use std::{ops::Deref, path::PathBuf, time::Duration};

use crate::cmd::CmdExe;
use crate::error::Error;
use crate::shared::ProcessMsg;
use rand::Rng;
use sysinfo::{CpuExt, DiskExt, Pid, PidExt, ProcessExt, SystemExt, UserExt};

// 定义Agent结构体
pub struct Agent {
    pub agent_id: String,
    pub version: String,
    pub host_name: String,
    pub token: String,
    pub nats_server: String,
    pub nats_ping_interval: u64,
    pub insecure_conf: bool,
    pub reqwest_client: reqwest::Client,
    pub system: sysinfo::System,
    pub py_bin: PathBuf,
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

    pub fn system_reboot_required(&self) -> bool {
        #[cfg(windows)]
        {
            let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
            hklm.open_subkey(r#"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired"#).is_ok()
        }
        #[cfg(not(windows))]
        {
            use std::path::Path;
            use tracing::debug;
            use xshell::cmd;
            // deb
            if ["/var/run/reboot-required", "/run/reboot-required"]
                .iter()
                .any(|p| Path::new(p).exists())
            {
                return true;
            }
            // // rhel
            if let Ok(sh) = xshell::Shell::new() {
                for bin in ["/usr/bin/needs-restarting", "/bin/needs-restarting"]
                    .iter()
                    .filter(|p| Path::new(p).exists())
                {
                    let cmd = cmd!(sh, "{bin} -r");
                    if let Err(e) = cmd.run() {
                        debug!("system_reboot_required(): {e:?}");
                        continue;
                    }
                    if let Ok(o) = cmd.output() {
                        if o.status.success() {
                            return true;
                        }
                    }
                }
            } else {
                debug!("system_reboot_required(): create xshell failed.");
            }

            false
        }
    }

    pub fn get_procs_rpc(&self) -> Vec<ProcessMsg> {
        self.system
            .processes()
            .into_iter()
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
                            .unwrap_or(&"")
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

    pub fn nats_options(&self) -> async_nats::ConnectOptions {
        let reconnect_wait = rand::thread_rng().gen_range(2..4);
        let mut conn_opts = async_nats::ConnectOptions::new()
            .name(&self.agent_id)
            .user_and_password(self.agent_id.clone(), self.token.clone())
            .reconnect_delay_callback(move |attempts| {
                Duration::from_secs(attempts as u64 * reconnect_wait)
            })
            .retry_on_initial_connect()
            .ping_interval(Duration::from_secs(self.nats_ping_interval));

        if !self.insecure_conf {
            conn_opts = conn_opts.require_tls(true);
        }

        conn_opts
    }
}

pub struct Status {
    pub stdout: String,
    pub stderr: String,
    pub exitcode: i32,
}

#[derive(serde::Deserialize)]
pub struct AgentConfig {
    pub base_url: String,
    pub agent_id: String,
    pub api_url: String,
    pub token: String,
    pub pk: i64,
    pub cert: String,
    pub proxy: String,
    pub nats_port: String,
    pub nats_ping_interval: i32,
    pub insecure: bool,
}

impl AgentConfig {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent() {
        let agent = Agent {
            agent_id: "".into(),
            token: "".into(),
            host_name: "".into(),
            nats_server: "".into(),
            version: "0.1".into(),
            nats_ping_interval: 10,
            insecure_conf: true,
            reqwest_client: reqwest::Client::new(),
            system: sysinfo::System::new_all(),
            py_bin: PathBuf::new(),
        };
        println!("{}", agent.get_cpu_usage());
        println!("{:?}", agent.get_load_avg());
        println!("{:?}", agent.get_disks());
        println!("{}", agent.logged_on_user());
        println!("{}", agent.os_string());
        println!("{}", agent.system_reboot_required());

        println!("{:?}", agent.get_procs_rpc());
    }
}

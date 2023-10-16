use crate::{
    error::Error, AgentInfoNats, CheckInNats, PublicIPNats, WinDisksNats, WinSvcNats, WinWMINats,
};
use sysinfo::SystemExt;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy)]
pub enum AgentMode {
    Hello,
    WinSvc,
    AgentInfo,
    WMI,
    Disks,
    PublicIp,
}

impl ToString for AgentMode {
    fn to_string(&self) -> String {
        match self {
            AgentMode::Hello => "agent-hello",
            AgentMode::WinSvc => "agent-winsvc",
            AgentMode::AgentInfo => "agent-agentinfo",
            AgentMode::WMI => "agent-wmi",
            AgentMode::Disks => "agent-disks",
            AgentMode::PublicIp => "agent-publicip",
        }
        .into()
    }
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

impl super::Agent {
    // func (a *Agent) NatsMessage(nc *nats.Conn, mode string) {
    pub async fn nats_message(
        &self,
        mode: AgentMode,
        client: &async_nats::Client,
        #[cfg(windows)] wmi: &crate::wmi::WmiManager,
    ) -> Result<(), Error> {
        use bytes::BufMut;
        let mut writer = bytes::BytesMut::new().writer();
        match mode {
            AgentMode::Hello => serde_json::to_writer(
                &mut writer,
                &CheckInNats {
                    agent_id: self.agent_id.clone(),
                    version: self.version.clone(),
                },
            ),
            AgentMode::WinSvc => serde_json::to_writer(
                &mut writer,
                &WinSvcNats {
                    agent_id: self.agent_id.clone(),
                    win_svcs: {
                        #[cfg(windows)]
                        {
                            crate::svc::get_service()?
                        }
                        #[cfg(not(windows))]
                        {
                            vec![]
                        }
                    },
                },
            ),
            AgentMode::AgentInfo => serde_json::to_writer(
                &mut writer,
                &AgentInfoNats {
                    agent_id: self.agent_id.clone(),
                    username: self.logged_on_user(),
                    hostname: self.host_name.clone(),
                    os: self.os_string(),
                    plat: std::env::consts::OS.into(),
                    total_ram: self.system.total_memory(),
                    boot_time: self.system.boot_time(),
                    reboot_needed: self.system_reboot_required().await,
                    arch: std::env::consts::ARCH.into(),
                },
            ),
            AgentMode::WMI => serde_json::to_writer(
                &mut writer,
                &WinWMINats {
                    agent_id: self.agent_id.clone(),
                    wmi: {
                        {
                            #[cfg(windows)]
                            {
                                let mut wmi = wmi.clone();
                                wmi.get_wmi_info().await?
                            }
                            #[cfg(not(windows))]
                            {
                                serde_json::Value::Null
                            }
                        }
                    },
                },
            ),
            AgentMode::Disks => serde_json::to_writer(
                &mut writer,
                &WinDisksNats {
                    agent_id: self.agent_id.clone(),
                    disks: self.get_disks(),
                },
            ),
            AgentMode::PublicIp => {
                let public_ip = crate::utils::public_ip().await?.to_string();
                serde_json::to_writer(
                    &mut writer,
                    &PublicIPNats {
                        agent_id: self.agent_id.clone(),
                        public_ip,
                    },
                )
            }
        }?;

        client
            .publish_with_reply(
                self.agent_id.clone(),
                serde_json::to_string(&mode).unwrap(),
                writer.into_inner().freeze(),
            )
            .await?;
        Ok(())
    }
}

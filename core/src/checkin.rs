use crate::{
    error::Error, AgentInfoNats, CheckInNats, PublicIPNats, WinDisksNats, WinSvcNats, WinWMINats,
};
use sysinfo::SystemExt;
use tracing::error;
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy)]
pub enum AgentMode {
    Hello,
    WinSvc,
    AgentInfo,
    WMI,
    Disks,
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

impl super::Agent {
    // func (a *Agent) NatsMessage(nc *nats.Conn, mode string) {
    pub async fn nats_message(
        &self,
        mode: AgentMode,
        client: &async_nats::Client,
        #[cfg(windows)] wmi: &tokio::sync::Mutex<crate::wmi::WmiManager>,
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
                    reboot_needed: self.system_reboot_required(),
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
                                let mut wmi = wmi.lock().await;
                                let info = wmi.get_wmi_info().await?;
                                info
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

    pub async fn do_nats_check_in(
        &self,
        #[cfg(windows)] wmi: &tokio::sync::Mutex<crate::wmi::WmiManager>,
    ) -> Result<(), Error> {
        let opts = self.nats_options();
        let nc = async_nats::connect_with_options(&self.nats_server, opts).await?;
        for m in AgentMode::all() {
            if let Err(e) = self
                .nats_message(
                    m,
                    &nc,
                    #[cfg(windows)]
                    wmi,
                )
                .await
            {
                error!("check nats: {m:?} failed: {e:?}");
            }
        }
        Ok(())
    }
}

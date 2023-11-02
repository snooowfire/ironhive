use crate::{
    agent::{logged_on_user, system_reboot_required},
    error::Error,
};
use shared::{
    AgentInfoNats, AgentMode, CheckInNats, PublicIPNats, WinDisksNats, WinSvcNats, WinWMINats,
};
use sysinfo::SystemExt;

impl crate::agent::Agent {
    pub async fn nats_message(
        &self,
        mode: AgentMode,
        client: &async_nats::Client,
        #[cfg(windows)] wmi: &crate::windows::wmi::WmiManager,
    ) -> Result<(), Error> {
        use bytes::BufMut;
        let mut writer = bytes::BytesMut::new().writer();
        match mode {
            AgentMode::Hello => serde_json::to_writer(
                &mut writer,
                &CheckInNats {
                    agent_id: self.agent_id.clone(),
                    version: self.version().into(),
                },
            ),
            AgentMode::WinSvc => serde_json::to_writer(
                &mut writer,
                &WinSvcNats {
                    agent_id: self.agent_id.clone(),
                    win_svcs: {
                        #[cfg(windows)]
                        {
                            crate::windows::svc::get_services()?
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
                    username: logged_on_user(),
                    hostname: self.host_name().into(),
                    os: self.os_string(),
                    plat: std::env::consts::OS.into(),
                    total_ram: self.system.total_memory(),
                    boot_time: self.system.boot_time(),
                    reboot_needed: system_reboot_required().await,
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
                mode.to_string(),
                writer.into_inner().freeze(),
            )
            .await?;
        Ok(())
    }
}

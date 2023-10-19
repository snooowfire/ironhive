use std::collections::HashMap;
use std::fmt::Display;
use std::time::Duration;

use crate::checkin::AgentMode;
use crate::cmd::{CmdScript, CmdShell, ScriptMode};
use crate::error::Error;
use crate::{agent::Agent, shared::*};
use async_nats::service::error::Error as NatsError;
use async_nats::ConnectOptions;
use bytes::Bytes;
use futures_util::StreamExt;
use sysinfo::PidExt;
use tracing::{debug, error, trace};

pub struct Ironhive {
    pub client: async_nats::Client,
    pub subscriber: async_nats::Subscriber,
    pub agent: Agent,
}

struct NatsClient<'c> {
    client: &'c async_nats::Client,
}

impl<'c> NatsClient<'c> {
    fn new(client: &'c async_nats::Client) -> Self {
        Self { client }
    }

    async fn respond(&self, msg: async_nats::Message, resp: &NatsResp) -> Result<(), Error> {
        if let Some(reply) = msg.reply {
            self.client.publish(reply, resp.as_bytes()).await?;
            Ok(())
        } else {
            Err(Error::NoReplySubject)
        }
    }

    async fn respond_res(
        &self,
        msg: async_nats::Message,
        resp: &Result<NatsResp, NatsError>,
    ) -> Result<(), Error> {
        match resp {
            Ok(payload) => {
                if let Some(reply) = msg.reply {
                    self.client.publish(reply, payload.as_bytes()).await
                } else {
                    return Err(Error::NoReplySubject);
                }
            }
            Err(err) => {
                let mut headers = async_nats::HeaderMap::new();
                headers.insert(async_nats::service::NATS_SERVICE_ERROR, err.status.as_str());
                headers.insert(
                    async_nats::service::NATS_SERVICE_ERROR_CODE,
                    err.code.to_string().as_str(),
                );
                if let Some(reply) = msg.reply {
                    self.client
                        .publish_with_headers(reply, headers, "".into())
                        .await
                } else {
                    return Err(Error::NoReplySubject);
                }
            }
        }?;

        Ok(())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "func")]
pub enum NatsMsg {
    Ping,
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
    Checkin {
        mode: AgentMode,
    },
    RebootNow,
    NeedsReboot,
    SysInfo,
    WMI,
    CpuLoadAvg,
    CpuUssage,
    PublicIp,
}

fn default_timeout() -> Duration {
    Duration::from_secs(15)
}

impl NatsMsg {
    pub fn as_bytes(&self) -> Bytes {
        use bytes::BufMut;
        let mut writer = bytes::BytesMut::new().writer();
        // TODO:
        // 覆蓋單元測試
        serde_json::to_writer(&mut writer, self).unwrap();
        writer.into_inner().freeze()
    }
}

impl Ironhive {
    pub async fn new(agent: Agent) -> Result<Self, Error> {
        let client = async_nats::connect(&agent.nats_server).await?;

        let subscriber = client.subscribe(agent.agent_id.clone()).await?;

        Ok(Self {
            client,
            subscriber,
            agent,
        })
    }

    pub async fn new_with_options(agent: Agent, options: ConnectOptions) -> Result<Self, Error> {
        let client = async_nats::connect_with_options(&agent.nats_server, options).await?;

        let subscriber = client.subscribe(agent.agent_id.clone()).await?;

        Ok(Self {
            client,
            subscriber,
            agent,
        })
    }

    pub async fn run(self) -> Result<(), Error> {
        trace!("start run.");
        let Self {
            client,
            mut subscriber,
            agent,
        } = self;

        #[cfg(windows)]
        let wmi = crate::wmi::WmiManager::init().await?;

        // TODO: safaty comments
        let mut scope = unsafe { async_scoped::TokioScope::create() };

        while let Some(msg) = subscriber.next().await {
            trace!("recv nats message: {:#?}", &msg);

            let agent = &agent;
            let client = &client;
            let nats_client = NatsClient::new(client);

            #[cfg(windows)]
            let wmi = &wmi;
            scope.spawn(async move {
                if let Ok(nats_msg) = serde_json::from_slice::<'_, NatsMsg>(&msg.payload) {
                    debug!("recv nats msg: {:?}", &nats_msg);
                    match nats_msg {
                        NatsMsg::Ping => {
                            if let Err(e) = nats_client.respond(msg, &NatsResp::Pong).await {
                                error!("Publish pong failed: {e:?}");
                            }
                        }
                        NatsMsg::Procs => {
                            if let Err(e) = nats_client
                                .respond(
                                    msg,
                                    &NatsResp::ProcessMsg {
                                        msgs: agent.get_procs_rpc(),
                                    },
                                )
                                .await
                            {
                                error!("Publish process msg failed: {e:?}");
                            }
                        }
                        NatsMsg::KillProc { proc_pid } => {
                            if let Err(e) = nats_client
                                .respond_res(
                                    msg,
                                    &NatsResp::ok(
                                        agent.kill_proc(sysinfo::Pid::from_u32(proc_pid)),
                                        |_| 0,
                                    ),
                                )
                                .await
                            {
                                error!("Kill process failed: {e:?}");
                            }
                        }
                        NatsMsg::RawCmd {
                            shell,
                            command,
                            timeout,
                            // run_as_user,
                        } => {
                            let cmd_shell = CmdShell {
                                shell,
                                command,
                                detached: false,
                                timeout,
                            };

                            if let Err(e) = nats_client
                                .respond_res(
                                    msg,
                                    &NatsResp::map_err(
                                        cmd_shell.run().await,
                                        |resp| NatsResp::RawCMDResp {
                                            results: {
                                                String::from_utf8_lossy(
                                                    &if resp.stderr.is_empty() {
                                                        resp.stdout
                                                    } else {
                                                        resp.stderr
                                                    },
                                                )
                                                .to_string()
                                            },
                                        },
                                        |_| 0,
                                    ),
                                )
                                .await
                            {
                                error!("Raw command failed: {e:?}");
                            }
                        }
                        NatsMsg::RunScript {
                            code,
                            mode,
                            script_args,
                            timeout,
                            // run_as_user,
                            env_vars,
                            id,
                        } => {
                            let now = std::time::Instant::now();
                            let cmd_script = CmdScript {
                                code,
                                mode,
                                args: script_args,
                                env_vars,
                                detached: false,
                                timeout,
                            };
                            let res = cmd_script.run().await;
                            debug!("{res:#?}");
                            let execution_time = std::time::Instant::now() - now;

                            if let Err(e) = nats_client
                                .respond_res(
                                    msg,
                                    &NatsResp::map_err(
                                        res,
                                        |resp| NatsResp::RunScriptResp {
                                            stdout: String::from_utf8_lossy(&resp.stdout)
                                                .to_string(),
                                            stderr: String::from_utf8_lossy(&resp.stderr)
                                                .to_string(),
                                            retcode: resp.status.code().unwrap_or(85),
                                            execution_time,
                                            id,
                                        },
                                        |_| 0,
                                    ),
                                )
                                .await
                            {
                                error!("Raw command failed: {e:?}");
                            }
                        }
                        NatsMsg::RebootNow => {
                            if let Err(e) = nats_client
                                .respond_res(msg, &NatsResp::ok(agent.reboot_now().await, |_| 0))
                                .await
                            {
                                error!("Reboot now failed: {e:?}");
                            }
                        }
                        NatsMsg::NeedsReboot => {
                            if let Err(e) = nats_client
                                .respond(
                                    msg,
                                    &NatsResp::NeedsReboot {
                                        needs: agent.system_reboot_required().await,
                                    },
                                )
                                .await
                            {
                                error!("Publish needs reboot failed: {e:?}");
                            }
                        }
                        NatsMsg::SysInfo => {
                            let res = async {
                                let modes = [
                                    AgentMode::AgentInfo,
                                    AgentMode::Disks,
                                    AgentMode::WMI,
                                    AgentMode::PublicIp,
                                ];
                                for mode in modes {
                                    agent
                                        .nats_message(
                                            mode,
                                            client,
                                            #[cfg(windows)]
                                            wmi,
                                        )
                                        .await?;
                                }
                                Result::<(), Error>::Ok(())
                            };
                            if let Err(e) = nats_client
                                .respond_res(msg, &NatsResp::ok(res.await, |_| 0))
                                .await
                            {
                                error!("Get sys info failed: {e:?}");
                            }
                        }
                        NatsMsg::WMI => {
                            let res = agent
                                .nats_message(
                                    AgentMode::WMI,
                                    client,
                                    #[cfg(windows)]
                                    wmi,
                                )
                                .await;
                            if let Err(e) = nats_client
                                .respond_res(msg, &NatsResp::ok(res, |_| 0))
                                .await
                            {
                                error!("Get WMI info failed: {e:?}");
                            };
                        }
                        NatsMsg::CpuLoadAvg => {
                            let res = agent.get_load_avg();
                            if let Err(e) = nats_client
                                .respond(
                                    msg,
                                    &NatsResp::CpuLoadAvg {
                                        one: res.one,
                                        five: res.five,
                                        fifteen: res.fifteen,
                                    },
                                )
                                .await
                            {
                                error!("Get cpu load avg failed: {e:?}");
                            }
                        }
                        NatsMsg::CpuUssage => {
                            if let Err(e) = nats_client
                                .respond(
                                    msg,
                                    &NatsResp::CpuUssage {
                                        usage: agent.get_cpu_usage(),
                                    },
                                )
                                .await
                            {
                                error!("Get cpu ussage failed: {e:?}");
                            }
                        }
                        NatsMsg::PublicIp => {
                            if let Err(e) = nats_client
                                .respond_res(
                                    msg,
                                    &NatsResp::map_err(
                                        crate::utils::public_ip().await,
                                        |resp| NatsResp::PublicIp {
                                            ip: resp.to_string(),
                                        },
                                        |_| 0,
                                    ),
                                )
                                .await
                            {
                                error!("Get public ip failed: {e:?}");
                            }
                        }
                        NatsMsg::Checkin { mode } => {
                            if let Err(e) = nats_client
                                .respond_res(
                                    msg,
                                    &NatsResp::ok(
                                        agent
                                            .nats_message(
                                                mode,
                                                client,
                                                #[cfg(windows)]
                                                wmi,
                                            )
                                            .await,
                                        |_| 0,
                                    ),
                                )
                                .await
                            {
                                error!("Checkin failed: {e:?}");
                            }
                        }
                    }
                } else {
                    trace!("Unknow request: {:?}", msg);
                }
            });
        }

        while let Some(handle) = scope.next().await {
            if let Err(e) = handle {
                error!("join handle failed: {e:?}");
            }
        }

        Ok(())
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "resp")]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub enum NatsResp {
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
}

impl NatsResp {
    pub fn ok<E: Display, F: for<'e> Fn(&'e E) -> usize>(
        res: Result<(), E>,
        code: F,
    ) -> Result<Self, NatsError> {
        if let Err(e) = res {
            Err(NatsError {
                code: code(&e),
                status: format!("{e}"),
            })
        } else {
            Ok(Self::Ok)
        }
    }
    pub fn map_err<T, E: Display, M: Fn(T) -> Self, F: for<'e> Fn(&'e E) -> usize>(
        res: Result<T, E>,
        map: M,
        code: F,
    ) -> Result<Self, NatsError> {
        match res {
            Ok(t) => Ok(map(t)),
            Err(e) => Err(NatsError {
                code: code(&e),
                status: format!("{e}"),
            }),
        }
    }

    pub fn as_bytes(&self) -> Bytes {
        use bytes::BufMut;
        let mut writer = bytes::BytesMut::new().writer();
        // TODO:
        // 覆蓋單元測試
        serde_json::to_writer(&mut writer, self).unwrap();
        writer.into_inner().freeze()
    }
}

#[test]
fn test_serialize_nats_resp() {
    let resp = NatsResp::Pong;
    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    let process_msg = NatsResp::ProcessMsg {
        msgs: vec![
            ProcessMsg {
                name: "cupnfish".into(),
                pid: 1023,
                membytes: 12345743,
                username: "Cupnfish".into(),
                id: "76".into(),
                cpu_percent: "7%".into(),
            },
            ProcessMsg {
                name: "cupnfish1".into(),
                pid: 1023,
                membytes: 12345743,
                username: "Cupnfish".into(),
                id: "76".into(),
                cpu_percent: "27%".into(),
            },
        ],
    };
    println!("{}", serde_json::to_string_pretty(&process_msg).unwrap());
}

use std::collections::HashMap;
use std::fmt::Display;
use std::time::Duration;

use crate::checkin::AgentMode;
use crate::cmd::{CmdScript, CmdShell, ScriptMode};
use crate::error::Error;
use crate::{agent::Agent, shared::*};
use async_nats::service::error::Error as NatsError;
use async_nats::service::Request;
use bytes::Bytes;
use futures_util::StreamExt;
use sysinfo::PidExt;
use tokio::sync::Mutex;
use tracing::{debug, error};

pub struct NatsHandler {
    client: async_nats::Client,
    endpoint: async_nats::service::endpoint::Endpoint,
    agent: Agent,
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
        timeout: Duration,
        run_as_user: bool,
    },
    RunScript {
        code: String,
        mode: ScriptMode,
        script_args: Vec<String>,
        timeout: Duration,
        run_as_user: bool,
        env_vars: HashMap<String, String>,
        id: i32,
    },
    RebootNow,
    NeedsReboot,
    SysInfo,
    WMI,
    CpuLoadAvg,
    CpuUssage,
    PublicIp,
}

impl NatsHandler {
    pub async fn run(self) -> Result<(), Error> {
        let Self {
            client,
            mut endpoint,
            agent,
        } = self;

        #[cfg(windows)]
        let wmi = Mutex::new(crate::wmi::WmiManager::init().await?);

        // TODO: safaty comments
        let mut scope = unsafe { async_scoped::TokioScope::create() };

        while let Some(msg) = endpoint.next().await {
            let agent = &agent;
            let client = &client;
            #[cfg(windows)]
            let wmi = &wmi;
            scope.spawn(async move {
                if let Ok(nats_msg) = serde_json::from_slice::<'_, NatsMsg>(&msg.message.payload) {
                    match nats_msg {
                        NatsMsg::Ping => {
                            if let Err(e) = respond(msg, &NatsResp::Pong).await {
                                error!("Publish pong failed: {e:?}");
                            }
                        }
                        NatsMsg::Procs => {
                            if let Err(e) = respond(
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
                            if let Err(e) = respond_res(
                                msg,
                                NatsResp::ok(
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
                            run_as_user,
                        } => {
                            let cmd_shell = CmdShell {
                                shell,
                                command,
                                detached: false,
                                timeout,
                            };

                            if let Err(e) = respond_res(
                                msg,
                                NatsResp::map_err(
                                    cmd_shell.run().await,
                                    |resp| NatsResp::RawCMDResp {
                                        results: {
                                            String::from_utf8_lossy(&if resp.stderr.is_empty() {
                                                resp.stdout
                                            } else {
                                                resp.stderr
                                            })
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
                            run_as_user,
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
                            let execution_time = std::time::Instant::now() - now;

                            if let Err(e) = respond_res(
                                msg,
                                NatsResp::map_err(
                                    res,
                                    |resp| NatsResp::RunScriptResp {
                                        stdout: String::from_utf8_lossy(&resp.stdout).to_string(),
                                        stderr: String::from_utf8_lossy(&resp.stdout).to_string(),
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
                            if let Err(e) =
                                respond_res(msg, NatsResp::ok(agent.reboot_now().await, |_| 0))
                                    .await
                            {
                                error!("Reboot now failed: {e:?}");
                            }
                        }
                        NatsMsg::NeedsReboot => {
                            if let Err(e) =
                                respond(msg, &NatsResp::NeedsReboot(agent.system_reboot_required()))
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
                                            &wmi,
                                        )
                                        .await?;
                                }
                                Result::<(), Error>::Ok(())
                            };
                            if let Err(e) = respond_res(msg, NatsResp::ok(res.await, |_| 0)).await {
                                error!("Get sys info failed: {e:?}");
                            }
                        }
                        NatsMsg::WMI => {
                            if let Err(e) = agent
                                .nats_message(
                                    AgentMode::WMI,
                                    client,
                                    #[cfg(windows)]
                                    &wmi,
                                )
                                .await
                            {
                                error!("Get WMI info failed: {e:?}");
                            };
                        }
                        NatsMsg::CpuLoadAvg => {
                            let res = agent.get_load_avg();
                            if let Err(e) = respond(
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
                            if let Err(e) = respond(
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
                            if let Err(e) = respond_res(
                                msg,
                                NatsResp::map_err(
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
                    }
                } else {
                    debug!("Unknow request: {:?}", msg.message);
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

async fn respond(request: Request, resp: &NatsResp) -> Result<(), Error> {
    request.respond(Ok(resp.as_bytes())).await?;
    Ok(())
}

async fn respond_res(request: Request, resp: Result<NatsResp, NatsError>) -> Result<(), Error> {
    request.respond(resp.map(|resp| resp.as_bytes())).await?;
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "resp")]
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
        execution_time: Duration,
        id: i32,
    },
    NeedsReboot(bool),
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
}

impl NatsResp {
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

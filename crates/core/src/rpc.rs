use crate::agent::Agent;
use crate::agent::{reboot_now, system_reboot_required};
use crate::cmd::{CmdScript, CmdShell};
use crate::error::Error;
#[cfg(windows)]
use crate::windows::wua::{get_win_updates, install_updates};
use async_nats::ConnectOptions;
use futures_util::StreamExt;
use sysinfo::PidExt;
use tracing::{debug, error, trace};

use shared::{as_bytes, AgentMode, IronhiveRequest, IronhiveRespond};

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

    /// Safaty: Ensure coverage of unit tests
    async unsafe fn respond_raw<T: serde::Serialize>(
        &self,
        msg: async_nats::Message,
        raw: &T,
    ) -> Result<(), Error> {
        if let Some(reply) = msg.reply {
            self.client.publish(reply, as_bytes(raw)).await?;
            Ok(())
        } else {
            Err(Error::NoReplySubject)
        }
    }

    async fn respond(&self, msg: async_nats::Message, resp: &IronhiveRespond) -> Result<(), Error> {
        unsafe { self.respond_raw(msg, resp) }.await
    }

    /// Safaty: Ensure coverage of unit tests
    async unsafe fn respond_res_raw<T: serde::Serialize>(
        &self,
        msg: async_nats::Message,
        raw_res: &Result<T, Error>,
    ) -> Result<(), Error> {
        match raw_res {
            Ok(raw) => {
                if let Some(reply) = msg.reply {
                    self.client.publish(reply, as_bytes(raw)).await
                } else {
                    return Err(Error::NoReplySubject);
                }
            }
            Err(err) => {
                let mut headers = async_nats::HeaderMap::new();
                headers.insert(
                    async_nats::service::NATS_SERVICE_ERROR,
                    format!("{err:?}").as_str(),
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

    async fn respond_res(
        &self,
        msg: async_nats::Message,
        resp: &Result<IronhiveRespond, Error>,
    ) -> Result<(), Error> {
        unsafe { self.respond_res_raw(msg, resp) }.await?;

        Ok(())
    }
}

impl Ironhive {
    pub async fn new(agent: Agent) -> Result<Self, Error> {
        let client = async_nats::connect(&agent.nats_servers).await?;

        let subscriber = client.subscribe(agent.agent_id.clone()).await?;

        Ok(Self {
            client,
            subscriber,
            agent,
        })
    }

    pub async fn new_with_options(agent: Agent, options: ConnectOptions) -> Result<Self, Error> {
        let client = async_nats::connect_with_options(&agent.nats_servers, options).await?;

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
        let wmi = crate::windows::wmi::WmiManager::init().await?;

        #[cfg(windows)]
        let get_win_update_locker = tokio::sync::Mutex::new(());
        // TODO: safaty comments
        let mut scope = unsafe { async_scoped::TokioScope::create() };

        debug!("start handle NATS message.");

        while let Some(msg) = subscriber.next().await {
            trace!("recv nats message: {:#?}", &msg);

            let agent = &agent;
            let client = &client;
            let nats_client = NatsClient::new(client);
            #[cfg(windows)]
            let wua_locker = &get_win_update_locker;
            #[cfg(windows)]
            let wmi = &wmi;
            scope.spawn(async move {
                if let Ok(nats_msg) = serde_json::from_slice::<'_, IronhiveRequest>(&msg.payload) {
                    debug!("recv nats msg: {:?}", &nats_msg);
                    handle_request(
                        nats_msg,
                        nats_client,
                        msg,
                        agent,
                        client,
                        #[cfg(windows)]
                        wmi,
                        #[cfg(windows)]
                        wua_locker,
                    )
                    .await;

                    if let Err(e) = client.flush().await {
                        error!("Flush NATS client failed: {e:?}");
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

async fn handle_request(
    nats_msg: IronhiveRequest,
    nats_client: NatsClient<'_>,
    msg: async_nats::Message,
    agent: &Agent,
    client: &async_nats::Client,
    #[cfg(windows)] wmi: &crate::windows::wmi::WmiManager,
    #[cfg(windows)] wua_locker: &tokio::sync::Mutex<()>,
) {
    match nats_msg {
        IronhiveRequest::Ping => {
            if let Err(e) = nats_client.respond(msg, &IronhiveRespond::Pong).await {
                error!("Publish pong failed: {e:?}");
            }
        }
        IronhiveRequest::Procs => {
            if let Err(e) = nats_client
                .respond(
                    msg,
                    &IronhiveRespond::ProcessMsg {
                        msgs: agent.get_procs_rpc(),
                    },
                )
                .await
            {
                error!("Publish process msg failed: {e:?}");
            }
        }
        IronhiveRequest::KillProc { proc_pid } => {
            if let Err(e) = nats_client
                .respond_res(
                    msg,
                    &agent
                        .kill_proc(sysinfo::Pid::from_u32(proc_pid))
                        .map(|_| IronhiveRespond::Ok),
                )
                .await
            {
                error!("Kill process failed: {e:?}");
            }
        }
        IronhiveRequest::RawCmd {
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
                    &cmd_shell
                        .run()
                        .await
                        .map(|resp| IronhiveRespond::RawCMDResp {
                            results: {
                                String::from_utf8_lossy(&if resp.stderr.is_empty() {
                                    resp.stdout
                                } else {
                                    resp.stderr
                                })
                                .to_string()
                            },
                        }),
                )
                .await
            {
                error!("Raw command failed: {e:?}");
            }
        }
        IronhiveRequest::RunScript {
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
                    &res.map(|resp| IronhiveRespond::RunScriptResp {
                        stdout: String::from_utf8_lossy(&resp.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&resp.stderr).to_string(),
                        retcode: resp.status.code().unwrap_or(85),
                        execution_time,
                        id,
                    }),
                )
                .await
            {
                error!("Raw command failed: {e:?}");
            }
        }
        IronhiveRequest::RebootNow => {
            if let Err(e) = nats_client
                .respond_res(msg, &reboot_now().await.map(|_| IronhiveRespond::Ok))
                .await
            {
                error!("Reboot now failed: {e:?}");
            }
        }
        IronhiveRequest::NeedsReboot => {
            if let Err(e) = nats_client
                .respond(
                    msg,
                    &IronhiveRespond::NeedsReboot {
                        needs: system_reboot_required().await,
                    },
                )
                .await
            {
                error!("Publish needs reboot failed: {e:?}");
            }
        }
        IronhiveRequest::SysInfo => {
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
                .respond_res(msg, &res.await.map(|_| IronhiveRespond::Ok))
                .await
            {
                error!("Get sys info failed: {e:?}");
            }
        }
        IronhiveRequest::WMI => {
            let res = agent
                .nats_message(
                    AgentMode::WMI,
                    client,
                    #[cfg(windows)]
                    wmi,
                )
                .await;
            if let Err(e) = nats_client
                .respond_res(msg, &res.map(|_| IronhiveRespond::Ok))
                .await
            {
                error!("Get WMI info failed: {e:?}");
            };
        }
        IronhiveRequest::CpuLoadAvg => {
            let res = agent.get_load_avg();
            if let Err(e) = nats_client
                .respond(
                    msg,
                    &IronhiveRespond::CpuLoadAvg {
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
        IronhiveRequest::CpuUssage => {
            if let Err(e) = nats_client
                .respond(
                    msg,
                    &IronhiveRespond::CpuUssage {
                        usage: agent.get_cpu_usage(),
                    },
                )
                .await
            {
                error!("Get cpu ussage failed: {e:?}");
            }
        }
        IronhiveRequest::PublicIp => {
            if let Err(e) = nats_client
                .respond_res(
                    msg,
                    &crate::utils::public_ip()
                        .await
                        .map(|resp| IronhiveRespond::PublicIp {
                            ip: resp.to_string(),
                        }),
                )
                .await
            {
                error!("Get public ip failed: {e:?}");
            }
        }
        IronhiveRequest::Checkin { mode } => {
            if let Err(e) = nats_client
                .respond_res(
                    msg,
                    &agent
                        .nats_message(
                            mode,
                            client,
                            #[cfg(windows)]
                            wmi,
                        )
                        .await
                        .map(|_| IronhiveRespond::Ok),
                )
                .await
            {
                error!("Checkin failed: {e:?}");
            }
        }
        IronhiveRequest::SoftwareList => {
            #[cfg(windows)]
            let res = crate::windows::svc::installed_software_list()
                .map(|software| IronhiveRespond::WinSoftwareNats { software });
            #[cfg(not(windows))]
            let res = Err(Error::UnsupportedRequest("SoftwareList".into()));

            if let Err(e) = nats_client.respond_res(msg, &res).await {
                error!("get installed software list failed: {e:?}");
            }
        }
        IronhiveRequest::InstallChoco => {
            #[cfg(windows)]
            let res = crate::windows::choco::install_choco()
                .await
                .map(|_| IronhiveRespond::Ok);
            #[cfg(not(windows))]
            let res = Err(Error::UnsupportedRequest("InstallChoco".into()));

            if let Err(e) = nats_client.respond_res(msg, &res).await {
                error!("Checkin failed: {e:?}");
            }
        }
        #[allow(unused_variables)]
        IronhiveRequest::InstallWithChoco { choco_prog_name } => {
            #[cfg(windows)]
            let res = {
                let now = std::time::Instant::now();
                let res = crate::windows::choco::install_with_choco(choco_prog_name).await;
                debug!("{res:#?}");
                let execution_time = std::time::Instant::now() - now;
                res.map(|resp| IronhiveRespond::RunScriptResp {
                    stdout: String::from_utf8_lossy(&resp.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&resp.stderr).to_string(),
                    retcode: resp.status.code().unwrap_or(85),
                    execution_time,
                    id: -1,
                })
            };

            #[cfg(not(windows))]
            let res = Err(Error::UnsupportedRequest("InstallWithChoco".into()));

            if let Err(e) = nats_client.respond_res(msg, &res).await {
                error!("Raw command failed: {e:?}");
            }
        }
        IronhiveRequest::PatchMgmt { patch_mgmnt } => {
            let res = crate::agent::patch_mgmnt(patch_mgmnt).map(|_| IronhiveRespond::Ok);

            if let Err(e) = nats_client.respond_res(msg, &res).await {
                error!("PatchMgmt failed: {e:?}");
            }
        }
        IronhiveRequest::WinServices => {
            #[cfg(windows)]
            let res = {
                crate::windows::svc::get_services()
                    .map(|services| IronhiveRespond::WinServices { services })
            };

            #[cfg(not(windows))]
            let res = Err(Error::UnsupportedRequest("WinServices".into()));

            if let Err(e) = nats_client.respond_res(msg, &res).await {
                error!("WinServices failed: {e:?}");
            }
        }
        #[allow(unused_variables)]
        IronhiveRequest::WinSvcDetail { name } => {
            #[cfg(windows)]
            let res = {
                crate::windows::svc::get_service_detail(name)
                    .map(|service| IronhiveRespond::WinSvcDetail { service })
            };

            #[cfg(not(windows))]
            let res = Err(Error::UnsupportedRequest("WinSvcDetail".into()));

            if let Err(e) = nats_client.respond_res(msg, &res).await {
                error!("WinSvcDetail failed: {e:?}");
            }
        }
        #[allow(unused_variables)]
        IronhiveRequest::WinSvcAction { name, action } => {
            #[cfg(windows)]
            let res = {
                crate::windows::svc::control_service(name, action).map_err(|service| {
                    IronhiveRespond::WinSvcResp {
                        success: false,
                        errormsg: format!("{service:?}"),
                    }
                })
            };

            #[cfg(not(windows))]
            let res: Result<(), IronhiveRespond> = Ok(());

            // Unwrapping `Result<IronhiveRespond, IronhiveRespond>` here should not cause any issues.
            let resp = res
                .map(|_| IronhiveRespond::WinSvcResp {
                    success: true,
                    errormsg: "".into(),
                })
                .unwrap();

            if let Err(e) = nats_client.respond(msg, &resp).await {
                error!("WinSvcAction failed: {e:?}");
            }
        }
        #[allow(unused_variables)]
        IronhiveRequest::EditWinSvc { name, start_type } => {
            #[cfg(windows)]
            let res = {
                crate::windows::svc::edit_service(name, start_type).map_err(|service| {
                    IronhiveRespond::WinSvcResp {
                        success: false,
                        errormsg: format!("{service:?}"),
                    }
                })
            };

            #[cfg(not(windows))]
            let res: Result<(), IronhiveRespond> = Ok(());

            // Unwrapping `Result<IronhiveRespond, IronhiveRespond>` here should not cause any issues.
            let resp = res
                .map(|_| IronhiveRespond::WinSvcResp {
                    success: true,
                    errormsg: "".into(),
                })
                .unwrap();

            if let Err(e) = nats_client.respond(msg, &resp).await {
                error!("EditWinSvc failed: {e:?}");
            }
        }
        IronhiveRequest::GetWinUpdates => {
            #[cfg(windows)]
            let res = {
                let guard = wua_locker.try_lock();
                if guard.is_err() {
                    let err = windows::core::Error::new(
                        windows::core::Error::OK.into(),
                        "Already installing or checking for windows updates".into(),
                    );
                    Err(Error::WindowsError(err))
                } else {
                    let updates = get_win_updates();
                    drop(guard);
                    updates
                }
            };
            #[cfg(not(windows))]
            let res = Ok(vec![]);

            let resp = res.map(|pkgs| IronhiveRespond::WinUpdateResult { updates: pkgs });

            if let Err(e) = nats_client.respond_res(msg, &resp).await {
                error!("GetWinUpdates failed: {e:?}");
            }
        }
        #[allow(unused_variables)]
        IronhiveRequest::InstallWinUpdates { update_guids } => {
            #[cfg(windows)]
            let res = {
                let guard = wua_locker.try_lock();
                if guard.is_err() {
                    let err = windows::core::Error::new(
                        windows::core::Error::OK.into(),
                        "Already installing or checking for windows updates".into(),
                    );
                    Err(Error::WindowsError(err))
                } else {
                    let updates = install_updates(update_guids)
                        .map(|needs| IronhiveRespond::NeedsReboot { needs });
                    drop(guard);
                    updates
                }
            };

            #[cfg(not(windows))]
            let res = Err(Error::UnsupportedRequest("InstallWinUpdates".into()));

            if let Err(e) = nats_client.respond_res(msg, &res).await {
                error!("InstallWinUpdates failed: {e:?}");
            }
        }
    }
}

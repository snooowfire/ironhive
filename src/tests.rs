#[cfg(feature = "deserialize")]
mod deserialize {
    use std::{collections::HashMap, time::Duration};

    use crate::{
        Agent, AgentInfoNats, AgentMode, CheckInNats, NatsMsg, NatsResp, PublicIPNats,
        WinDisksNats, WinSvcNats, WinWMINats,
    };
    use futures_util::StreamExt;
    use sysinfo::SystemExt;
    use tracing::{debug, info};
    use tracing_test::traced_test;

    #[traced_test]
    #[tokio::test(flavor = "multi_thread")]
    async fn basic() {
        let server = nats_server::run_basic_server();

        let agent_id = uuid::Uuid::new_v4();

        info!("agent id: {}", agent_id.to_string());

        let agent = Agent {
            agent_id: agent_id.to_string(),
            version: "0.1.0".into(),
            host_name: "ironhive".into(),
            nats_server: server.client_url(),
            ..Default::default()
        };

        let rpc = crate::Ironhive::new(agent).await.unwrap();

        let client = rpc.client.clone();

        let req = tokio::spawn(async move {
            let msgs = [
                NatsMsg::Ping,
                NatsMsg::CpuLoadAvg,
                NatsMsg::CpuUssage,
                NatsMsg::NeedsReboot,
                NatsMsg::Procs,
                NatsMsg::PublicIp,
                NatsMsg::SysInfo,
            ];
            tokio::time::sleep(Duration::from_secs(2)).await;

            for msg in msgs {
                client
                    .publish_with_reply(agent_id.to_string(), "ironhive".into(), msg.as_bytes())
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });

        let client = rpc.client.clone();

        let resp = async {
            let mut subscriber = client.subscribe("ironhive".into()).await.unwrap();

            macro_rules! check_resp {
                ($($resp:pat),*) => {{
                    $(
                        let msg = subscriber.next().await.unwrap();
                        debug!("resp msg: {msg:?}");
                        let resp =  serde_json::from_slice::<NatsResp>(&msg.payload).unwrap();
                        debug!("resp: {resp:?}");
                        assert!(matches!(
                           resp,
                            $resp
                        ));
                    )*
                }};
            }

            check_resp!(
                NatsResp::Pong,
                NatsResp::CpuLoadAvg { .. },
                NatsResp::CpuUssage { .. },
                NatsResp::NeedsReboot { .. },
                NatsResp::ProcessMsg { .. },
                NatsResp::PublicIp { .. },
                NatsResp::Ok
            );
        };

        let tasks = async {
            let _ = tokio::join!(req, resp);
        };

        tokio::select! {
            res = rpc.run() => res.unwrap(),
            _ = tasks => {
                info!("well");
            },
        }
    }

    #[traced_test]
    #[tokio::test(flavor = "multi_thread")]
    async fn checkin() {
        let server = nats_server::run_basic_server();

        let agent_id = uuid::Uuid::new_v4();

        info!("agent id: {}", agent_id.to_string());

        let agent = Agent {
            agent_id: agent_id.to_string(),
            version: "0.1.0".into(),
            host_name: "ironhive".into(),
            nats_server: server.client_url(),
            ..Default::default()
        };

        let rpc = crate::Ironhive::new(agent).await.unwrap();

        let client = rpc.client.clone();

        let req = tokio::spawn(async move {
            let msgs = AgentMode::all().map(|mode| NatsMsg::Checkin { mode });

            tokio::time::sleep(Duration::from_secs(2)).await;

            for msg in msgs {
                client
                    .publish_with_reply(agent_id.to_string(), "ironhive".into(), msg.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let client = rpc.client.clone();

        let oks = async {
            let mut subscriber = client.subscribe("ironhive".into()).await.unwrap();

            let len = AgentMode::all().len();

            for _ in 0..len {
                let raw_resp = subscriber.next().await.unwrap();
                debug!("{raw_resp:#?}");
                let resp = serde_json::from_slice::<NatsResp>(&raw_resp.payload).unwrap();
                assert!(matches!(resp, NatsResp::Ok));
            }
        };

        macro_rules! checkin {
            ($($name: ident = $agent_mode: path => $resp: ty),*) => {
                $(
                    let $name = async {
                        let mut subscriber = client
                        .subscribe(serde_json::to_string(&$agent_mode).unwrap())
                        .await
                        .unwrap();

                        let raw_resp = subscriber.next().await.unwrap();

                        let res = serde_json::from_slice::<$resp>(&raw_resp.payload);

                        assert!(res.is_ok());

                        debug!("{:#?}",res.unwrap());
                    };
                )*
            };
        }

        checkin! {hello = AgentMode::Hello => CheckInNats,
            winsvc = AgentMode::WinSvc => WinSvcNats,
            agent_info = AgentMode::AgentInfo => AgentInfoNats,
            wmi = AgentMode::WMI => WinWMINats,
            disks = AgentMode::Disks => WinDisksNats,
            public_ip = AgentMode::PublicIp => PublicIPNats
        };

        let tasks = async {
            let _ = tokio::join!(req, oks, hello, winsvc, agent_info, wmi, disks, public_ip);
        };

        tokio::select! {
            res = rpc.run() => res.unwrap(),
            _ = tasks => {
                info!("well");
            },
        }
    }

    #[traced_test]
    #[tokio::test(flavor = "multi_thread")]
    async fn wmi() {
        let server = nats_server::run_basic_server();

        let agent_id = uuid::Uuid::new_v4();

        info!("agent id: {}", agent_id.to_string());

        let agent = Agent {
            agent_id: agent_id.to_string(),
            version: "0.1.0".into(),
            host_name: "ironhive".into(),
            nats_server: server.client_url(),
            ..Default::default()
        };

        let rpc = crate::Ironhive::new(agent).await.unwrap();

        let client = rpc.client.clone();

        let req = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            client
                .publish_with_reply(
                    agent_id.to_string(),
                    "ironhive".into(),
                    NatsMsg::WMI.as_bytes(),
                )
                .await
                .unwrap();
        });

        let client = rpc.client.clone();

        let ok = async {
            let mut subscriber = client.subscribe("ironhive".into()).await.unwrap();

            let raw_resp = subscriber.next().await.unwrap();
            let resp = serde_json::from_slice::<NatsResp>(&raw_resp.payload).unwrap();
            assert!(matches!(resp, NatsResp::Ok));
        };

        let wmi = async {
            let mut subscriber = client
                .subscribe(serde_json::to_string(&AgentMode::WMI).unwrap())
                .await
                .unwrap();

            let raw_resp = subscriber.next().await.unwrap();

            let res = serde_json::from_slice::<WinWMINats>(&raw_resp.payload);

            assert!(res.is_ok());

            debug!("{:#?}", res.unwrap());
        };

        let tasks = async {
            let _ = tokio::join!(req, ok, wmi);
        };

        tokio::select! {
            res = rpc.run() => res.unwrap(),
            _ = tasks => {
                info!("well");
            },
        }
    }

    #[traced_test]
    #[tokio::test(flavor = "multi_thread")]
    async fn basic_command() {
        let server = nats_server::run_basic_server();

        let agent_id = uuid::Uuid::new_v4();

        info!("agent id: {}", agent_id.to_string());

        let agent = Agent {
            agent_id: agent_id.to_string(),
            version: "0.1.0".into(),
            host_name: "ironhive".into(),
            nats_server: server.client_url(),
            ..Default::default()
        };

        let mut rpc = crate::Ironhive::new(agent).await.unwrap();

        let client = rpc.client.clone();
        let test_server = nats_server::run_basic_server();

        tokio::time::sleep(Duration::from_secs(2)).await;

        let test_pid = test_server.client_pid();

        rpc.agent.system.refresh_all();

        use sysinfo::ProcessExt;

        let name = rpc
            .agent
            .system
            .process(sysinfo::Pid::from(test_pid))
            .map(|p| p.name())
            .unwrap();

        let expect_name = {
            #[cfg(windows)]
            {
                "nats-server.exe"
            }
            #[cfg(not(windows))]
            {
                "nats-server"
            }
        };

        assert_eq!(name, expect_name);

        let req = tokio::spawn(async move {
            let msgs = [
                NatsMsg::KillProc {
                    proc_pid: test_pid as u32,
                },
                NatsMsg::RawCmd {
                    shell: "cmd".into(),
                    command: "cargo --help".into(),
                    timeout: Duration::from_secs(2),
                },
                NatsMsg::RawCmd {
                    shell: "powershell".into(),
                    command: "cargo --help".into(),
                    timeout: Duration::from_secs(2),
                },
                NatsMsg::RawCmd {
                    shell: "bash".into(),
                    command: "cargo --help".into(),
                    timeout: Duration::from_secs(2),
                },
            ];

            tokio::time::sleep(Duration::from_secs(2)).await;

            for msg in msgs {
                let reply = {
                    if let NatsMsg::RawCmd { ref shell, .. } = msg {
                        shell.clone()
                    } else {
                        "ironhive".into()
                    }
                };
                client
                    .publish_with_reply(agent_id.to_string(), reply, msg.as_bytes())
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });

        let client = rpc.client.clone();

        let resp = async {
            macro_rules! check_resp {
                ($($subject: literal = $resp:pat $(in $flog: literal)? $(=> $slog: literal)?),*) => {{
                    $(
                        let mut subscriber = client.subscribe($subject.into()).await.unwrap();

                        let msg = subscriber.next().await.unwrap();
                        debug!("resp msg: {msg:?}");
                        if let Ok(resp) =  serde_json::from_slice::<NatsResp>(&msg.payload){
                            debug!("resp: {resp:?}");
                            assert!(matches!(
                               resp,
                                $resp
                            ));
                            $(assert!(logs_contain($slog));)?
                        }else {
                            assert!(msg.payload.is_empty());
                            let headers = msg.headers.unwrap();
                            assert!(headers.get(async_nats::service::NATS_SERVICE_ERROR).is_some());
                            assert!(headers.get(async_nats::service::NATS_SERVICE_ERROR_CODE).is_some());
                            $(let error = headers.get(async_nats::service::NATS_SERVICE_ERROR).unwrap();
                            assert!(error.as_str().contains($flog)))?
                        }

                    )*
                }};
            }

            check_resp!(
                "ironhive" = NatsResp::Ok,
                "cmd" = NatsResp::RawCMDResp { .. } in "io error" => "cargo -Z help",
                "powershell" = NatsResp::RawCMDResp { .. } in "io error" => "cargo -Z help",
                "bash" = NatsResp::RawCMDResp { .. } in "unsupported shell" => "cargo -Z help"
            );
        };

        let tasks = async {
            let _ = tokio::join!(req, resp);
        };

        tokio::select! {
            res = rpc.run() => res.unwrap(),
            _ = tasks => {
                info!("well");
            },
        }
    }

    #[traced_test]
    #[tokio::test(flavor = "multi_thread")]
    async fn run_script() {
        let server = nats_server::run_basic_server();

        let agent_id = uuid::Uuid::new_v4();

        info!("agent id: {}", agent_id.to_string());

        let agent = Agent {
            agent_id: agent_id.to_string(),
            version: "0.1.0".into(),
            host_name: "ironhive".into(),
            nats_server: server.client_url(),
            ..Default::default()
        };

        let rpc = crate::Ironhive::new(agent).await.unwrap();

        let client = rpc.client.clone();

        let req = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            client
                .publish_with_reply(
                    agent_id.to_string(),
                    "ironhive".into(),
                    NatsMsg::RunScript {
                        code: r#"print("hi from ironhive!")"#.into(),
                        mode: crate::cmd::ScriptMode::Python {
                            bin: "python3".into(),
                        },
                        script_args: vec![],
                        timeout: Duration::from_secs(3),
                        env_vars: HashMap::new(),
                        id: 1,
                    }
                    .as_bytes(),
                )
                .await
                .unwrap();
            client
                .publish_with_reply(
                    agent_id.to_string(),
                    "ironhive".into(),
                    NatsMsg::RunScript {
                        code: r#"
def fibonacci(n):
    if n <= 0:
        return "Please enter a positive integer."
    elif n == 1:
        return 0
    elif n == 2:
        return 1
    else:
        a, b = 0, 1
        for _ in range(3, n+1):
            a, b = b, a + b
        return b

n = 10
result = fibonacci(n)
print(f"The value of the {n}th term in the Fibonacci sequence is: {result}")
                "#
                        .into(),
                        mode: crate::cmd::ScriptMode::Python {
                            bin: "python3".into(),
                        },
                        script_args: vec![],
                        timeout: Duration::from_secs(10),
                        env_vars: HashMap::new(),
                        id: 2,
                    }
                    .as_bytes(),
                )
                .await
                .unwrap();
        });

        let client = rpc.client.clone();

        let res = async {
            let mut subscriber = client.subscribe("ironhive".into()).await.unwrap();

            let raw_resp = subscriber.next().await.unwrap();
            let resp = serde_json::from_slice::<NatsResp>(&raw_resp.payload).unwrap();
            assert!(matches!(resp, NatsResp::RunScriptResp { .. }));

            fn handle_resp(stdout: String, id: i32) {
                if id == 1 {
                    assert_eq!(stdout.trim(), "hi from ironhive!");
                } else if id == 2 {
                    assert_eq!(
                        stdout.trim(),
                        "The value of the 10th term in the Fibonacci sequence is: 34"
                    );
                } else {
                    panic!("Unknow resp");
                }
            }

            if let NatsResp::RunScriptResp { stdout, id, .. } = resp {
                handle_resp(stdout, id)
            }

            let raw_resp = subscriber.next().await.unwrap();
            let resp = serde_json::from_slice::<NatsResp>(&raw_resp.payload).unwrap();
            assert!(matches!(resp, NatsResp::RunScriptResp { .. }));
            debug!("{resp:#?}");
            if let NatsResp::RunScriptResp { stdout, id, .. } = resp {
                handle_resp(stdout, id)
            }
        };

        let tasks = async {
            let _ = tokio::join!(req, res);
        };

        tokio::select! {
            res = rpc.run() => res.unwrap(),
            _ = tasks => {
                info!("well");
            },
        }
    }
}

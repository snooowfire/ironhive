use std::time::Duration;

use futures_util::StreamExt;
use ironhive_config::generate_agent_id;
use ironhive_core::{Agent, Ironhive, IronhiveRequest, IronhiveRespond};
use sysinfo::SystemExt;
use tracing::{debug, info};
use tracing_test::traced_test;

#[traced_test]
#[tokio::test(flavor = "multi_thread")]
async fn basic_command() {
    let server = nats_server::run_basic_server();

    let agent_id = generate_agent_id();

    info!("agent id: {}", agent_id.to_string());

    let agent = Agent::new(agent_id.to_string(), &server.client_url()).unwrap();

    let mut rpc = crate::Ironhive::new(agent).await.unwrap();

    let client = rpc.client.clone();
    let test_server = nats_server::run_basic_server();

    tokio::time::sleep(Duration::from_secs(2)).await;

    let test_pid = test_server.client_pid();

    rpc.agent.system.refresh_all();

    use sysinfo::ProcessExt;

    let name = rpc.agent.system.process(sysinfo::Pid::from(test_pid)).map(|p| p.name()).unwrap();

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
            IronhiveRequest::KillProc { proc_pid: test_pid as u32 },
            IronhiveRequest::RawCmd {
                shell: "cmd".into(),
                command: "cargo --help".into(),
                timeout: Duration::from_secs(2),
            },
            IronhiveRequest::RawCmd {
                shell: "powershell".into(),
                command: "cargo --help".into(),
                timeout: Duration::from_secs(2),
            },
            IronhiveRequest::RawCmd {
                shell: "bash".into(),
                command: "cargo --help".into(),
                timeout: Duration::from_secs(2),
            },
        ];

        tokio::time::sleep(Duration::from_secs(2)).await;

        for msg in msgs {
            let reply = {
                if let IronhiveRequest::RawCmd { ref shell, .. } = msg {
                    shell.clone()
                } else {
                    "ironhive".into()
                }
            };
            client.publish_with_reply(agent_id.to_string(), reply, msg.as_bytes()).await.unwrap();
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
                    if let Ok(resp) =  serde_json::from_slice::<IronhiveRespond>(&msg.payload){
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
                        $(let error = headers.get(async_nats::service::NATS_SERVICE_ERROR).unwrap();
                        assert!(error.as_str().contains($flog)))?
                    }

                )*
            }};
        }

        check_resp!(
            "ironhive" = IronhiveRespond::Ok,
            "cmd" = IronhiveRespond::RawCMDResp { .. } in "io error" => "cargo -Z help",
            "powershell" = IronhiveRespond::RawCMDResp { .. } in "io error" => "cargo -Z help",
            "bash" = IronhiveRespond::RawCMDResp { .. } in "bash" => "cargo -Z help"
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

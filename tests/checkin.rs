use std::time::Duration;

use futures_util::StreamExt;
use ironhive_config::generate_agent_id;
use ironhive_core::{
    Agent, AgentInfoNats, AgentMode, CheckInNats, Ironhive, IronhiveRequest, IronhiveRespond,
    PublicIPNats, WinDisksNats, WinSvcNats, WinWMINats,
};
use tracing::{debug, info};
use tracing_test::traced_test;

#[traced_test]
#[tokio::test(flavor = "multi_thread")]
async fn checkin() {
    let server = nats_server::run_basic_server();

    let agent_id = generate_agent_id();

    info!("agent id: {}", agent_id);

    let agent = Agent::new(agent_id.clone(), &server.client_url()).unwrap();

    let rpc = crate::Ironhive::new(agent).await.unwrap();

    let client = rpc.client.clone();
    let agent_id_clone = agent_id.clone();
    let req = tokio::spawn(async move {
        let msgs = AgentMode::all().map(|mode| IronhiveRequest::Checkin { mode });

        tokio::time::sleep(Duration::from_secs(2)).await;

        for msg in msgs {
            client
                .publish_with_reply(agent_id_clone.clone(), "ironhive".into(), msg.as_bytes())
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
            let resp = serde_json::from_slice::<IronhiveRespond>(&raw_resp.payload).unwrap();
            assert!(matches!(resp, IronhiveRespond::Ok));
        }
    };

    macro_rules! checkin {
        ($($name: ident = $agent_mode: path => $resp: ty),*) => {
            $(
                let $name = async {
                    let mut subscriber = client.subscribe(agent_id.clone()).await.unwrap();

                    while let Some(raw_resp) = subscriber.next().await {
                        if let Some(reply) = raw_resp.reply {
                            if reply.eq(&serde_json::to_string(&$agent_mode).unwrap()) {
                                let res = serde_json::from_slice::<$resp>(&raw_resp.payload);

                                assert!(res.is_ok());

                                debug!("{:#?}", res.unwrap());

                                break;
                            }
                        }
                    }
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

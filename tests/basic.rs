use std::time::Duration;

use futures_util::StreamExt;
use ironhive_config::generate_agent_id;
use ironhive_core::{Agent, Ironhive, IronhiveRequest, IronhiveRespond};
use tracing::{debug, info};
use tracing_test::traced_test;

#[traced_test]
#[tokio::test(flavor = "multi_thread")]
async fn basic() {
    let server = nats_server::run_basic_server();

    let agent_id = generate_agent_id();

    info!("agent id: {}", agent_id.to_string());

    let agent = Agent::new(agent_id.to_string(), &server.client_url()).unwrap();

    let rpc = crate::Ironhive::new(agent).await.unwrap();

    let client = rpc.client.clone();

    let req = tokio::spawn(async move {
        let msgs = [
            IronhiveRequest::Ping,
            IronhiveRequest::CpuLoadAvg,
            IronhiveRequest::CpuUssage,
            IronhiveRequest::NeedsReboot,
            IronhiveRequest::Procs,
            IronhiveRequest::PublicIp,
            IronhiveRequest::SysInfo,
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
                    let resp =  serde_json::from_slice::<IronhiveRespond>(&msg.payload).unwrap();
                    debug!("resp: {resp:?}");
                    assert!(matches!(
                       resp,
                        $resp
                    ));
                )*
            }};
        }

        check_resp!(
            IronhiveRespond::Pong,
            IronhiveRespond::CpuLoadAvg { .. },
            IronhiveRespond::CpuUssage { .. },
            IronhiveRespond::NeedsReboot { .. },
            IronhiveRespond::ProcessMsg { .. },
            IronhiveRespond::PublicIp { .. },
            IronhiveRespond::Ok
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

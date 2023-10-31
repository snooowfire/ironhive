use std::time::Duration;

use futures_util::StreamExt;
use ironhive_config::generate_agent_id;
use ironhive_core::{Agent, AgentMode, Ironhive, IronhiveRequest, IronhiveRespond, WinWMINats};
use tracing::{debug, info};
use tracing_test::traced_test;

#[traced_test]
#[tokio::test(flavor = "multi_thread")]
async fn wmi() {
    let server = nats_server::run_basic_server();

    let agent_id = generate_agent_id();

    info!("agent id: {}", agent_id.to_string());

    let agent = Agent::new(agent_id.to_string(), &server.client_url()).unwrap();

    let rpc = crate::Ironhive::new(agent).await.unwrap();

    let client = rpc.client.clone();
    let agent_id_clone = agent_id.clone();
    let req = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        client
            .publish_with_reply(
                agent_id_clone,
                "ironhive".into(),
                IronhiveRequest::WMI.as_bytes(),
            )
            .await
            .unwrap();
    });

    let client = rpc.client.clone();

    let ok = async {
        let mut subscriber = client.subscribe("ironhive".into()).await.unwrap();

        let raw_resp = subscriber.next().await.unwrap();
        let resp = serde_json::from_slice::<IronhiveRespond>(&raw_resp.payload).unwrap();
        assert!(matches!(resp, IronhiveRespond::Ok));
    };

    let wmi = async {
        let mut subscriber = client.subscribe(agent_id).await.unwrap();

        while let Some(raw_resp) = subscriber.next().await {
            if let Some(reply) = raw_resp.reply {
                if reply.eq(&serde_json::to_string(&AgentMode::WMI).unwrap()) {
                    let res = serde_json::from_slice::<WinWMINats>(&raw_resp.payload);

                    assert!(res.is_ok());

                    debug!("{:#?}", res.unwrap());

                    break;
                }
            }
        }
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

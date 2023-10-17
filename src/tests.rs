#[cfg(feature = "deserialize")]
mod deserialize {
    use std::time::Duration;

    use crate::{Agent, NatsMsg, NatsResp};
    use futures_util::StreamExt;
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

        let last = tokio::spawn(async move {
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

        tokio::select! {
            res = rpc.run() => res.unwrap(),
            last = last => last.unwrap(),
            _ = async {
                // tokio::time::sleep(Duration::from_secs(1)).await;

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
            } => {
                info!("well");
            },
        }
    }
}

use ironhive::{Agent, AgentMode};
use tokio::join;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), ironhive::Error> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::DEBUG)
        .init();

    let agent_id = uuid::Uuid::new_v4();

    info!("agent id: {}", agent_id.to_string());

    let agent = Agent {
        agent_id: agent_id.to_string(),
        version: "0.1.0".into(),
        host_name: "ironhive".into(),
        nats_server: "nats://localhost:4222".into(),
        ..Default::default()
    };

    let rpc = ironhive::Ironhive::new(agent).await?;

    let client = rpc.client.clone();

    let _ = join!(rpc.run(), do_nats_check_in(agent_id, &client));

    Ok(())
}

async fn do_nats_check_in(
    agent_id: uuid::Uuid,
    client: &async_nats::Client,
) -> Result<(), ironhive::Error> {
    for (reply, m) in AgentMode::all().map(|mode| {
        (
            serde_json::to_string(&mode).unwrap(),
            ironhive::NatsMsg::Checkin { mode },
        )
    }) {
        client
            .publish_with_reply(agent_id.to_string(), reply, m.as_bytes())
            .await?
    }
    Ok(())
}

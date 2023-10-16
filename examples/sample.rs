use ironhive::Agent;
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

    rpc.run().await?;

    Ok(())
}

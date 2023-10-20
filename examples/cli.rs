use async_nats::ConnectOptions;
use ironhive::Agent;
use tracing::{info, Level};

use clap::Parser;

/// Simple Command Line Examples for IronHive
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Id of the ironhive to publish
    #[arg(short, long)]
    agent_id: String,

    /// Server of nats to connect
    #[arg(short, long)]
    server: String,
}

#[tokio::main]
async fn main() -> Result<(), ironhive::Error> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::DEBUG)
        .init();

    let args = Args::parse();

    let agent = Agent {
        agent_id: args.agent_id,
        version: "0.1.0".into(),
        host_name: "ironhive".into(),
        nats_server: args.server,
        ..Default::default()
    };

    let rpc = ironhive::Ironhive::new_with_options(
        agent,
        ConnectOptions::new().retry_on_initial_connect(),
    )
    .await?;
    info!("will run ironhive!");
    rpc.run().await?;
    info!("ironhive error!");

    Ok(())
}

use std::{collections::HashMap, time::Duration};

use async_nats::ConnectOptions;
use ironhive::{NatsMsg, ScriptMode};

use tracing::{info, Level};

use clap::Parser;

/// Simple Command Line Examples for IronHive
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Id of the ironhive to publish
    #[arg(long)]
    agent_id: String,

    /// Server of nats to connect
    #[arg(long)]
    server: String,

    /// Reply of the ironhive to publish
    #[arg(long)]
    reply: String,

    /// Python path of the ironhive to publish
    #[arg(long, default_value_t = String::from("python3"))]
    python: String,

    /// Python code of the ironhive to publish
    #[arg(long, short)]
    code: String,

    /// Timeout of the python code to run, secs
    #[arg(long)]
    timeout: u64,

    /// Script id
    #[arg(long)]
    id: i32,
}

#[tokio::main]
async fn main() -> Result<(), ironhive::Error> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(Level::DEBUG)
        .init();

    let args = Args::parse();

    let client = async_nats::connect_with_options(
        args.server,
        ConnectOptions::new().retry_on_initial_connect(),
    )
    .await?;

    client
        .publish_with_reply(
            args.agent_id,
            args.reply,
            NatsMsg::RunScript {
                code: args.code,
                mode: ScriptMode::Python {
                    bin: args.python.into(),
                },
                script_args: vec![],
                timeout: Duration::from_secs(args.timeout),
                env_vars: HashMap::new(),
                id: args.id,
            }
            .as_bytes(),
        )
        .await
        .unwrap();

    client.flush().await.unwrap();

    info!("publish python code fine!");

    Ok(())
}

use async_nats::ConnectOptions;
use futures_util::StreamExt;
use ironhive::NatsResp;

use tracing::Level;

use clap::Parser;

/// Simple Command Line Examples for IronHive
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Server of nats to connect
    #[arg(long)]
    server: String,

    /// Reply of the ironhive to publish
    #[arg(long)]
    reply: String,

    /// Expect output
    #[arg(long)]
    expect: String,

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

    let mut subscriber = client.subscribe(args.reply).await?;

    let msg = subscriber.next().await.expect("expect msg!!!");

    let resp = serde_json::from_slice::<NatsResp>(&msg.payload)?;

    if let NatsResp::RunScriptResp { stdout, id, .. } = resp {
        assert_eq!(id, args.id);
        assert_eq!(stdout.trim(), args.expect.trim());
    } else {
        panic!("Not expect stdout: {resp:#?}");
    }

    Ok(())
}

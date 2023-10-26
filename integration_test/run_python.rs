use std::{collections::HashMap, time::Duration};

use ironhive::{NatsMsg, ScriptMode};

use tracing::{info, Level};

use clap::Parser;

/// Simple Command Line Examples for IronHive
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Reply of the ironhive to publish
    #[arg(long)]
    reply: String,

    /// Python path of the ironhive to publish
    #[arg(long, default_value_t = String::from("python3"))]
    python: String,

    /// Timeout of the python code to run, secs
    #[arg(long)]
    timeout: u64,

    /// Script id
    #[arg(long)]
    id: i32,
}

#[tokio::main]
async fn main() -> Result<(), ironhive::Error> {
    tracing_subscriber::fmt().with_level(true).with_max_level(Level::DEBUG).init();

    let args = Args::parse();

    let config = ironhive::IronhiveConfig::new().unwrap();

    let (agent, opts) = config.agent_and_options().await?;

    let client = async_nats::connect_with_options(agent.nats_servers, opts).await?;

    client
        .publish_with_reply(agent.agent_id.clone(), "hello".into(), NatsMsg::Ping.as_bytes())
        .await?;

    info!("ping {:?} from run python.", agent.agent_id);

    let code = r#"
def fibonacci(n):
    if n <= 0:
        return "Please enter a positive integer."
    elif n == 1:
        return 0
    elif n == 2:
        return 1
    else:
        a, b = 0, 1
        for _ in range(3, n+1):
            a, b = b, a + b
        return b

n = 10
result = fibonacci(n)
print(f"The value of the {n}th term in the Fibonacci sequence is: {result}")
                    "#;

    client
        .publish_with_reply(
            agent.agent_id,
            args.reply,
            NatsMsg::RunScript {
                code: code.into(),
                mode: ScriptMode::Binary { path: args.python.into(), ext: ".py".into() },
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

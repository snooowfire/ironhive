use config::Source;
use ironhive_config::{environment, IronhiveConfig};
use tracing::Level;

use clap::{Parser, Subcommand};

mod install;

/// Simple Command Line Examples for IronHive
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Install IronHive program and optionally create default configuration.
    Install {
        /// Specify the addresses of NATS servers for IronHive to connect to.
        #[arg(short, long)]
        nats_servers: Vec<String>,

        /// Enable or disable overwriting the existing default configuration file.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        overwrite_config: bool,
    },
    /// Run IronHive.
    /// Note: IronHive must be installed using the 'install' command before running.
    Rpc,
    Env,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_level(true).with_max_level(Level::TRACE).init();

    let args = Args::parse();

    if let Some(cmd) = args.command {
        match cmd {
            Commands::Install { nats_servers, overwrite_config } => {
                let installer = install::Installer { nats_servers, overwrite_config };

                installer.install().await?;
            }
            Commands::Rpc => {
                let config = IronhiveConfig::new()?;

                let (agent, options) = config.agent_and_options().await?;

                let rpc = ironhive_core::Ironhive::new_with_options(agent, options).await?;

                rpc.run().await?;
            }
            Commands::Env => {
                let env = environment();
                let envs = env.collect()?;
                for (k, v) in envs {
                    println!("{k} = {v}");
                }
            }
        }
    }

    Ok(())
}

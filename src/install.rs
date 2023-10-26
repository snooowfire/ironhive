use anyhow::{anyhow, Ok, Result};
use tokio::io::AsyncWriteExt;
use tracing::debug;

use ironhive_config::{default_config_json, generate_agent_id, proj_dirs, IronhiveConfig};

pub struct Installer {
    pub nats_servers: Vec<String>,
    pub overwrite_config: bool,
}

impl Installer {
    pub async fn install(self) -> Result<()> {
        if self.nats_servers.is_empty() {
            return Err(anyhow!("At least one NATS server address must be set."));
        }

        #[cfg(windows)]
        if !ironhive_core::is_root() {
            return Err(anyhow!("Must run as root."));
        }

        if self.overwrite_config {
            check_existing_and_remove().await?;
        }

        let agent_id = generate_agent_id();

        debug!("Agent ID: {agent_id}");

        let config = IronhiveConfig::init(self.nats_servers, agent_id);

        let proj_dirs = proj_dirs()?;

        let config_dir = proj_dirs.config_dir();

        if !config_dir.exists() {
            tokio::fs::create_dir_all(config_dir).await?;
        }

        let default_config_path = default_config_json(&proj_dirs);

        if !default_config_path.exists() {
            let mut default_config = tokio::fs::File::create(default_config_path).await?;

            default_config.write_all(&serde_json::to_vec_pretty(&config)?).await?;
        }

        Ok(())
    }
}

async fn check_existing_and_remove() -> Result<()> {
    let proj_dirs = proj_dirs()?;

    let config_dir = proj_dirs.config_dir();

    if config_dir.exists() {
        tokio::fs::remove_dir_all(config_dir).await?;
    }

    Ok(())
}

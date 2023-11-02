use std::{env, io, path::PathBuf, time::Duration};

use async_nats::ConnectOptions;
use config::{Config, ConfigError, Environment, File};
use directories::ProjectDirs;
use ironhive_core::Agent;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct IronhiveConfig {
    /// NATS Server URLs
    addrs: Vec<String>,
    /// exe path
    pub exe_path: PathBuf,
    /// username
    pub agent_id: String,
    /// password
    #[serde(skip_serializing_if = "Option::is_none")]
    pass: Option<String>,
    /// token
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    /// NKey Seed secret
    #[serde(skip_serializing_if = "Option::is_none")]
    nkey: Option<String>,
    /// credentials file
    #[serde(skip_serializing_if = "Option::is_none")]
    credentials_file: Option<PathBuf>,
    /// root certificates
    #[serde(skip_serializing_if = "Option::is_none")]
    root_certificates: Option<PathBuf>,
    /// client certificate cert
    #[serde(skip_serializing_if = "Option::is_none")]
    client_certificate_cert: Option<PathBuf>,
    /// client certificate cert
    #[serde(skip_serializing_if = "Option::is_none")]
    client_certificate_key: Option<PathBuf>,
    /// TLS requirement
    ///
    /// Default is set to disables.
    #[serde(skip_serializing_if = "Option::is_none")]
    require_tls: Option<bool>,
    /// How often Client sends PING message to the server
    #[serde(skip_serializing_if = "Option::is_none")]
    ping_interval: Option<Duration>,
    /// disables delivering messages that were lished from the same connection
    #[serde(skip_serializing_if = "Option::is_none")]
    no_echo: Option<bool>,
    /// The capacity for `Subscribers`
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription_capacity: Option<usize>,
    /// A timeout for the underlying TcpStream connection to avoid hangs and deadlocks.
    ///
    /// Default is set to 5 seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    connection_timeout: Option<Duration>,
    /// The capacity of client dispatches op's to the Client onto the channel
    ///
    /// Default value is set to 128.
    #[serde(skip_serializing_if = "Option::is_none")]
    client_capacity: Option<usize>,
    /// ignore discovered servers
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_discovered_servers: Option<bool>,
    /// retain servers order
    #[serde(skip_serializing_if = "Option::is_none")]
    retain_servers_order: Option<bool>,
    /// The initial capacity of the read buffer
    #[serde(skip_serializing_if = "Option::is_none")]
    read_buffer_capacity: Option<u16>,
}

pub fn proj_dirs() -> Result<ProjectDirs, ConfigError> {
    let proj_dirs = ProjectDirs::from("", "snooowfire", "ironhive")
        .ok_or(ConfigError::NotFound("ProjectDirs".into()))?;
    Ok(proj_dirs)
}

fn default_config(proj_dirs: &ProjectDirs) -> PathBuf {
    proj_dirs.config_dir().join("default")
}

pub fn default_config_json(proj_dirs: &ProjectDirs) -> PathBuf {
    let mut path = proj_dirs.config_dir().join("default");
    path.set_extension("json");
    path
}

pub fn environment() -> Environment {
    Environment::with_prefix("ironhive")
}

impl IronhiveConfig {
    pub fn init(addrs: Vec<String>, agent_id: String) -> Self {
        Self {
            addrs,
            agent_id,
            ..Default::default()
        }
    }

    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let proj_dirs = proj_dirs()?;

        let builder = Config::builder()
            .add_source(File::from(default_config(&proj_dirs)))
            .add_source(File::from(proj_dirs.config_local_dir().join(run_mode)).required(false))
            .add_source(environment());

        let s = builder.build()?;

        s.try_deserialize()
    }

    pub async fn agent_and_options(
        mut self,
    ) -> Result<(Agent, ConnectOptions), ironhive_core::Error> {
        let agent = Agent::with_servers(self.agent_id.clone(), self.addrs.drain(..));

        let options = self.connect_options().await?;

        Ok((agent, options))
    }

    async fn connect_options(self) -> Result<ConnectOptions, io::Error> {
        let mut options = ConnectOptions::new()
            .retry_on_initial_connect()
            .event_callback(|e| async move {
                tracing::debug!("nats event: {e:?}");
            });

        options = options.name(self.agent_id.clone());

        if let Some((cert, key)) = self
            .client_certificate_cert
            .zip(self.client_certificate_key)
        {
            options = options.add_client_certificate(cert, key);
        }

        if let Some(pass) = self.pass {
            options = options.user_and_password(self.agent_id, pass);
        }

        if let Some(root_certificates) = self.root_certificates {
            options = options.add_root_certificates(root_certificates);
        }

        if let Some(credentials_file) = self.credentials_file {
            options = options.credentials_file(credentials_file).await?;
        }

        macro_rules! set_options {
            ($($name:ident),*) => {
                $(
                    if let Some($name) = self.$name {
                        options = options.$name($name);
                    }
                )*
            };
        }

        set_options!(
            token,
            connection_timeout,
            nkey,
            require_tls,
            ping_interval,
            subscription_capacity,
            client_capacity,
            read_buffer_capacity
        );

        macro_rules! set_options_if {
            ($($name:ident),*) => {
                $(
                    if let Some(flag) = self.$name {
                        if flag {
                            options = options.$name();
                        }
                    }
                )*
            };
        }

        set_options_if!(no_echo, ignore_discovered_servers, retain_servers_order);

        Ok(options)
    }
}

pub fn generate_agent_id() -> String {
    let letters: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
        .chars()
        .collect();
    let mut rng = rand::thread_rng();
    let b: Vec<char> = (0..40)
        .map(|_| letters[rng.gen_range(0..letters.len())])
        .collect();
    b.iter().collect()
}

#[test]
fn test_serialize_config() {
    let config = IronhiveConfig {
        agent_id: generate_agent_id(),
        ..Default::default()
    };

    let json_res = serde_json::to_string_pretty(&config);

    assert!(json_res.is_ok());

    if let Ok(json) = json_res {
        tracing::debug!("{json}");
    }
}

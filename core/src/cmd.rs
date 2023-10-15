use std::{collections::HashMap, ffi::OsStr, path::PathBuf, process::Output, time::Duration};

use tokio::process::Command;

use crate::{error::Error, Agent};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CmdOptions<P = String, A = String, K = String, V = String> {
    /// have a child process that is in a different process group so that
    /// parent terminating doesn't kill child
    pub detached: bool,
    pub program: P,
    pub args: Vec<A>,
    pub env_vars: Vec<(K, V)>,
    pub timeout: Duration,
}

impl<P, A, K, V> CmdOptions<P, A, K, V>
where
    P: AsRef<OsStr>,
    A: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    fn command(self) -> (Command, Duration) {
        let Self {
            detached,
            program,
            args,
            env_vars,
            timeout,
        } = self;

        let mut cmd = Command::new(program);
        cmd.args(args).envs(env_vars);

        if detached {
            #[cfg(windows)]
            {
                use ::windows::Win32::System::Threading::{
                    CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
                };
                cmd.creation_flags((DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP).0);
            }
            #[cfg(unix)]
            {
                // TODO:
                cmd.gid(0);
            }
        }
        (cmd, timeout)
    }
    async fn run(self) -> Result<Output, Error> {
        let (mut cmd, timeout) = self.command();

        let output = tokio::time::timeout(timeout, cmd.output()).await??;

        Ok(output)
    }

    #[cfg(windows)]
    async fn run_with_raw(self, raw: impl AsRef<std::ffi::OsStr>) -> Result<Output, Error> {
        let (mut cmd, timeout) = self.command();

        cmd.raw_arg(raw);

        let output = tokio::time::timeout(timeout, cmd.output()).await??;

        Ok(output)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CmdShell<S = String, C = String> {
    pub shell: S,
    pub command: C,
    pub detached: bool,
    pub timeout: Duration,
}

impl<S, C> CmdShell<S, C>
where
    S: AsRef<OsStr>,
    C: AsRef<OsStr> + for<'s> From<&'s str>,
{
    pub async fn run(self) -> Result<Output, Error> {
        let Self {
            shell,
            command,
            detached,
            timeout,
        } = self;
        #[cfg(windows)]
        {
            let shell = shell.as_ref();
            if shell.eq("cmd") {
                return CmdOptions {
                    detached,
                    program: shell,
                    args: vec!["/C"],
                    env_vars: empty_vec(),
                    timeout,
                }
                .run_with_raw(command)
                .await;
            } else if shell.eq("powershell") {
                return CmdOptions {
                    detached,
                    program: shell,
                    args: vec!["-NonInteractive".into(), "-NoProfile".into(), command],
                    env_vars: empty_vec(),
                    timeout,
                }
                .run()
                .await;
            } else {
                return Err(Error::UnsupportedShell(shell.to_string_lossy().to_string()));
            }
        }
        #[cfg(not(windows))]
        {
            CmdOptions {
                detached,
                program: shell,
                args: vec!["-c".into(), command],
                env_vars: empty_vec(),
                timeout,
            }
            .run()
            .await
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CmdArgs<S = String, A = String> {
    pub shell: S,
    pub cmd_args: Vec<A>,
    pub detached: bool,
    pub timeout: Duration,
}

impl<S, A> CmdArgs<S, A>
where
    S: AsRef<OsStr> + for<'s> From<&'s str>,
    A: AsRef<OsStr> + for<'s> From<&'s str> + Clone,
{
    pub async fn run(self) -> Result<Output, Error> {
        let Self {
            shell,
            cmd_args,
            detached,
            timeout,
        } = self;
        #[cfg(windows)]
        {
            let shell = shell.as_ref();
            let (program, args) = if shell.eq("cmd") {
                let mut args = vec!["/C".into()];
                args.extend_from_slice(&cmd_args);
                (get_cmd_exe(), args)
            } else if shell.eq("powershell") {
                let mut args = vec!["-NonInteractive".into(), "-NoProfile".into()];
                args.extend_from_slice(&cmd_args);
                (get_powershell_exe(), args)
            } else {
                return Err(Error::UnsupportedShell(shell.to_string_lossy().to_string()));
            };

            return CmdOptions {
                detached,
                program,
                args,
                env_vars: empty_vec(),
                timeout,
            }
            .run()
            .await;
        }
        #[cfg(not(windows))]
        {
            return CmdOptions {
                detached,
                program: shell,
                args: cmd_args,
                env_vars: empty_vec(),
                timeout,
            }
            .run()
            .await;
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CmdExe<E = String, A = String> {
    pub exe: E,
    pub args: Vec<A>,
    pub detached: bool,
    pub timeout: Duration,
}

impl<E, A> CmdExe<E, A>
where
    E: AsRef<OsStr>,
    A: AsRef<OsStr>,
{
    pub async fn run(self) -> Result<Output, Error> {
        let Self {
            exe,
            args,
            detached,
            timeout,
        } = self;

        return CmdOptions {
            detached,
            program: exe,
            args,
            env_vars: empty_vec(),
            timeout,
        }
        .run()
        .await;
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CmdScript<K = String, V = String>
where
    K: std::cmp::Eq + std::hash::Hash,
{
    pub code: String,
    pub mode: ScriptMode,
    pub args: Vec<String>,
    pub env_vars: HashMap<K, V>,
    pub detached: bool,
    pub timeout: Duration,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ScriptMode {
    PowerShell,
    Python { bin: PathBuf },
    CMD,
    Directly,
}

impl ScriptMode {
    pub fn ext(&self) -> &str {
        match self {
            ScriptMode::PowerShell => ".ps1",
            ScriptMode::Python { .. } => ".py",
            ScriptMode::CMD => ".bat",
            ScriptMode::Directly => "",
        }
    }
}

impl<K, V> CmdScript<K, V>
where
    K: AsRef<OsStr> + std::cmp::Eq + std::hash::Hash,
    V: AsRef<OsStr>,
{
    pub async fn run(self) -> Result<Output, Error> {
        let Self {
            code,
            mode,
            args,
            env_vars,
            detached,
            timeout,
        } = self;
        let code = code.trim();

        let ext = mode.ext();

        let sh = xshell::Shell::new()?;
        let temp_dir = sh.create_temp_dir()?;

        let temp_file = {
            loop {
                let mut buffer = itoa::Buffer::new();
                let file_name = buffer.format(rand::random::<u64>()).to_string() + ext;

                let res = temp_dir.path().join(file_name);
                if !res.exists() {
                    break res;
                }
            }
        };
        sh.write_file(&temp_file, code)?;

        #[cfg(windows)]
        {
            CmdOptions {
                detached,
                args: {
                    match mode {
                        ScriptMode::PowerShell => {
                            let mut tmp = vec![
                                "-NonInteractive".into(),
                                "-NoProfile".into(),
                                "-ExecutionPolicy".into(),
                                "Bypass".into(),
                                temp_file.to_string_lossy().to_string(),
                            ];
                            tmp.extend_from_slice(&args);
                            tmp
                        }
                        ScriptMode::Python { .. } => {
                            let mut tmp = vec![temp_file.to_string_lossy().to_string()];
                            tmp.extend_from_slice(&args);
                            tmp
                        }
                        ScriptMode::CMD => args,
                        ScriptMode::Directly => args,
                    }
                },
                program: {
                    match mode {
                        ScriptMode::PowerShell => get_powershell_exe(),
                        ScriptMode::Python { bin } => bin,
                        ScriptMode::CMD => get_cmd_exe(),
                        ScriptMode::Directly => temp_file,
                    }
                },
                env_vars: env_vars.into_iter().collect(),
                timeout,
            }
            .run()
            .await
        }
        #[cfg(not(windows))]
        {
            match mode {
                ScriptMode::Python { bin } => CmdOptions {
                    detached,
                    program: bin,
                    args: {
                        let mut tmp = vec![temp_file.to_string_lossy().to_string()];
                        tmp.extend_from_slice(&args);
                        tmp
                    },
                    env_vars: env_vars.into_iter().collect(),
                    timeout,
                },
                _ => CmdOptions {
                    detached,
                    program: temp_file,
                    args,
                    env_vars: env_vars.into_iter().collect(),
                    timeout,
                },
            }
            .run()
            .await
        }
    }
}

#[cfg(windows)]
fn get_powershell_exe() -> std::path::PathBuf {
    use tracing::debug;

    if let Ok(output) = std::process::Command::new("powershell.exe").output() {
        if output.status.success() {
            // powershell.exe found
            return std::path::PathBuf::from("powershell.exe");
        }
    }
    debug!("get powershell exe fallback");
    // powershell.exe not found, fallback to default path
    std::path::PathBuf::from(std::env::var("WINDIR").unwrap())
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
}

#[cfg(windows)]
fn get_cmd_exe() -> std::path::PathBuf {
    use tracing::debug;

    if let Ok(output) = std::process::Command::new("cmd.exe").output() {
        if output.status.success() {
            // powershell.exe found
            return std::path::PathBuf::from("cmd.exe");
        }
    }
    debug!("get cmd exe fallback");
    // cmd.exe not found, fallback to default path
    std::path::PathBuf::from(std::env::var("WINDIR").unwrap())
        .join("System32")
        .join("cmd.exe")
}

fn empty_vec() -> Vec<(&'static str, &'static str)> {
    vec![]
}

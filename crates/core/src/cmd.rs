use std::{collections::HashMap, ffi::OsStr, process::Output, time::Duration};

use shared::ScriptMode;
use tokio::process::Command;

use crate::error::Error;

#[derive(Debug)]
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

pub struct CmdShell<S = String, C = String> {
    pub shell: S,
    pub command: C,
    pub detached: bool,
    pub timeout: Duration,
}

impl<S, C> CmdShell<S, C>
where
    S: AsRef<OsStr>,
    C: AsRef<OsStr>,
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
                CmdOptions {
                    detached,
                    program: shell,
                    args: vec!["/C"],
                    env_vars: empty_vec(),
                    timeout,
                }
                .run_with_raw(command)
                .await
            } else if shell.eq("powershell") {
                CmdOptions {
                    detached,
                    program: shell,
                    args: vec![
                        "-NonInteractive".as_ref(),
                        "-NoProfile".as_ref(),
                        command.as_ref(),
                    ],
                    env_vars: empty_vec(),
                    timeout,
                }
                .run()
                .await
            } else {
                Err(Error::UnsupportedShell(shell.to_string_lossy().to_string()))
            }
        }
        #[cfg(not(windows))]
        {
            CmdOptions {
                detached,
                program: shell,
                args: vec!["-c".as_ref(), command.as_ref()],
                env_vars: empty_vec(),
                timeout,
            }
            .run()
            .await
        }
    }
}

#[derive(Debug)]
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

        CmdOptions {
            detached,
            program: exe,
            args,
            env_vars: empty_vec(),
            timeout,
        }
        .run()
        .await
    }
}

pub struct CmdScript<C = String, A = String, K = String, V = String>
where
    K: std::cmp::Eq + std::hash::Hash,
{
    pub code: C,
    pub mode: ScriptMode,
    pub args: Vec<A>,
    pub env_vars: HashMap<K, V>,
    pub detached: bool,
    pub timeout: Duration,
}

impl<C, A, K, V> CmdScript<C, A, K, V>
where
    C: AsRef<str>,
    A: AsRef<OsStr>,
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

        let mut args = args.iter().map(|a| a.as_ref()).collect::<Vec<_>>();

        let ext = mode.ext();

        let temp_file = crate::temp_file::TempFile::new(ext, code.as_ref().trim()).await?;
        let temp_file_path = temp_file.path();
        let res = {
            #[cfg(windows)]
            {
                CmdOptions {
                    detached,
                    args: {
                        match mode {
                            ScriptMode::PowerShell => {
                                let mut tmp = vec![
                                    "-NonInteractive".as_ref(),
                                    "-NoProfile".as_ref(),
                                    "-ExecutionPolicy".as_ref(),
                                    "Bypass".as_ref(),
                                    temp_file_path.as_ref(),
                                ];
                                tmp.append(&mut args);
                                tmp
                            }
                            ScriptMode::Binary { .. } => {
                                let mut tmp = vec![temp_file_path.as_ref()];
                                tmp.append(&mut args);
                                tmp
                            }
                            ScriptMode::Cmd => args,
                            ScriptMode::Directly => args,
                        }
                    },
                    program: {
                        match mode {
                            ScriptMode::PowerShell => get_powershell_exe(),
                            ScriptMode::Binary { path, .. } => path,
                            ScriptMode::Cmd => get_cmd_exe(),
                            ScriptMode::Directly => temp_file_path.to_path_buf(),
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
                    ScriptMode::Binary { path, .. } => CmdOptions {
                        detached,
                        program: path,
                        args: {
                            let mut tmp = vec![temp_file_path.as_ref()];
                            tmp.append(&mut args);
                            tmp
                        },
                        env_vars: env_vars.into_iter().collect(),
                        timeout,
                    },
                    _ => CmdOptions {
                        detached,
                        program: temp_file_path.to_path_buf(),
                        args,
                        env_vars: env_vars.into_iter().collect(),
                        timeout,
                    },
                }
                .run()
                .await
            }
        };

        drop(temp_file);
        res
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

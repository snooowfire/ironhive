use sysinfo::Pid;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not set Agent `nats_options` value.")]
    NotFoundNatsConnectOptions,
    #[error("err pattern has separator: '/'")]
    PatternHasSeparator,
    #[error("No reply subject to reply to")]
    NoReplySubject,
    #[error("io error: {0}")]
    TokioIoError(#[from] tokio::io::Error),
    #[error("timeout: {0}")]
    Elapsed(#[from] tokio::time::error::Elapsed),
    #[error("run cmd error: {0:?}")]
    CmdError(tokio::process::ChildStderr),
    #[error("run python code failed.")]
    RunPythonCodeErr,
    #[error("nats connect failed: {0}")]
    NatsConnectError(#[from] async_nats::ConnectError),
    #[error("nats subscribe failed: {0}")]
    NatsSubscribeError(#[from] async_nats::SubscribeError),
    #[error("nats publish failed: {0}")]
    NatsPublishError(#[from] async_nats::PublishError),
    #[error("not found process: {0:?}")]
    NotFoundProcess(Pid),
    #[error("kill process: {0:?} failed.")]
    KillProcessFailed(Pid),
    #[error("unsupported shell: {0}")]
    UnsupportedShell(String),
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("serde json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("not found public ip")]
    NotFoundPublicIp,
    #[error("setup rpc failed.")]
    SetupRpcFailed,
    #[cfg(windows)]
    #[error("windows error: {0}")]
    WindowsError(#[from] windows::core::Error),
    #[cfg(windows)]
    #[error("wmi error: {0}")]
    WmiError(#[from] wmi::WMIError),
    #[error("from utf-16 failed: {0}")]
    FromUtf16Error(#[from] std::string::FromUtf16Error),
    #[error("from utf-8 failed: {0}")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("broadcast recv failed: {0}")]
    BroadcastRecvError(#[from] tokio::sync::broadcast::error::RecvError),
    #[error("oneshot recv failed: {0}")]
    OneshotRecvError(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("async nats error: {0}")]
    AsyncNatsError(#[from] async_nats::Error),
}

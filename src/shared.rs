use std::time::Duration;

#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub struct ProcessMsg {
    pub name: String,
    pub pid: u32,
    pub membytes: u64,
    pub username: String,
    pub id: String,
    pub cpu_percent: String,
}

#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub struct RawCMDResp {
    pub results: String,
}

#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub struct RunScriptResp {
    pub stdout: String,
    pub stderr: String,
    pub retcode: i32,
    pub execution_time: Duration,
    pub id: i32,
}

use std::time::Duration;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ProcessMsg {
    pub name: String,
    pub pid: u32,
    pub membytes: u64,
    pub username: String,
    pub id: String,
    pub cpu_percent: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RawCMDResp {
    pub results: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RunScriptResp {
    pub stdout: String,
    pub stderr: String,
    pub retcode: i32,
    pub execution_time: Duration,
    pub id: i32,
}

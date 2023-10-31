use shared::ScriptMode;

use crate::{
    cmd::{CmdExe, CmdScript},
    error::Error,
};
use std::{path::PathBuf, process::Output, time::Duration};

pub async fn install_choco() -> Result<(), Error> {
    let response = reqwest::get("https://chocolatey.org/install.ps1").await?;

    let run_script = CmdScript::<String, String> {
        code: response.text().await?,
        mode: ScriptMode::PowerShell,
        args: vec![],
        env_vars: Default::default(),
        detached: false,
        timeout: Duration::from_secs(999),
    }
    .run()
    .await?;

    if !run_script.status.success() {
        tracing::error!(
            "install choco failed: {:?}",
            String::from_utf8_lossy(&run_script.stderr)
        );
        return Err(Error::RunScriptFailed(ScriptMode::PowerShell));
    }

    Ok(())
}

pub async fn install_with_choco(name: String) -> Result<Output, Error> {
    let choco = PathBuf::from(env!("PROGRAMDATA"))
        .join("chocolatey")
        .join("bin")
        .join("choco.exe");

    let out = CmdExe {
        exe: choco,
        detached: false,
        timeout: Duration::from_secs(1200),
        args: vec![
            "install",
            name.as_str(),
            "--yes",
            "--force",
            "--force-dependencies",
            "--no-progress",
        ],
    }
    .run()
    .await?;

    Ok(out)
}

use anyhow::Result;

use crate::install::check_existing_and_remove;

pub struct Uninstaller;

impl Uninstaller {
    pub async fn uninstall() -> Result<()> {
        #[cfg(windows)]
        if !ironhive_core::is_root() {
            return Err(anyhow::anyhow!("Must run as root."));
        }

        check_existing_and_remove().await?;

        #[cfg(windows)]
        uninstall_service()?;

        Ok(())
    }
}

#[cfg(windows)]
fn uninstall_service() -> Result<()> {
    let uninstaller = ironhive_core::ServiceUninstaller {
        name: "ironhive".into(),
    };

    uninstaller.uninstall_service()?;

    Ok(())
}

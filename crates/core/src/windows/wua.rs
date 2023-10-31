use std::time::Duration;

use tracing::debug;
use windows::{
    core::{w, ComInterface, IUnknown, IntoParam, BSTR, GUID, PCWSTR},
    Win32::{
        Foundation::{S_FALSE, S_OK},
        System::{
            Com::{
                CLSIDFromProgID, CLSIDFromString, CoCreateInstance, CoInitializeEx, CLSCTX_SERVER,
                COINIT_MULTITHREADED,
            },
            UpdateAgent::{IUpdate, IUpdateCollection, IUpdateSession},
        },
    },
};

use crate::{agent::system_reboot_required_win, error::Error};
use shared::WUAPackage;

struct UpdateSession {
    inner: IUpdateSession,
}

impl UpdateSession {
    fn new() -> Result<Self, Error> {
        debug!("Create update session.");

        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) } {
            if e.code().is_err() || (e.code().ne(&S_FALSE) && e.code().ne(&S_OK)) {
                return Err(Error::WindowsError(e));
            }
        }

        let unknow = create_object(w!("Microsoft.Update.Session"))?;

        debug!("New update session well");

        Ok(Self {
            inner: unknow.cast()?,
        })
    }

    fn download_wuaupdate_collection(&self, updates: &IUpdateCollection) -> Result<(), Error> {
        let downloader = unsafe { self.inner.CreateUpdateDownloader() }?;

        unsafe { downloader.SetUpdates(updates) }?;

        let _ = unsafe { downloader.Download() }?;

        Ok(())
    }

    fn install_wuaupdate_collection(&self, updates: &IUpdateCollection) -> Result<(), Error> {
        let installer = unsafe { self.inner.CreateUpdateInstaller() }?;

        unsafe { installer.SetUpdates(updates) }?;

        let _ = unsafe { installer.Install() }?;

        Ok(())
    }

    fn install_wuaupdate(&self, updt: &IUpdate) -> Result<(), Error> {
        let _ = unsafe { updt.Title() }?;

        let updts = new_update_collection()?;

        let eula = unsafe { updt.EulaAccepted() }?;

        if !eula.as_bool() {
            unsafe { updt.AcceptEula() }?;
        }

        let _ = unsafe { updts.Add(updt) }?;

        self.download_wuaupdate_collection(&updts)?;

        self.install_wuaupdate_collection(&updts)?;

        Ok(())
    }

    fn get_wwuaupdate_collection(
        &self,
        query: impl IntoParam<BSTR>,
    ) -> Result<IUpdateCollection, Error> {
        let searcher = unsafe { self.inner.CreateUpdateSearcher() }?;
        let res = unsafe { searcher.Search(query) }?;

        let updts = unsafe { res.Updates() }?;

        Ok(updts)
    }
}

fn class_id_from(program_id: PCWSTR) -> Result<GUID, Error> {
    unsafe {
        Ok(match CLSIDFromProgID(program_id) {
            Err(_) => CLSIDFromString(program_id),
            ok => ok,
        }?)
    }
}

fn create_object(program_id: PCWSTR) -> Result<IUnknown, Error> {
    let class_id = class_id_from(program_id)?;

    let unknow = create_unknow_instance(&class_id)?;

    Ok(unknow)
}

fn create_unknow_instance(clsid: &GUID) -> Result<IUnknown, Error> {
    let hr = unsafe { CoCreateInstance::<_, IUnknown>(clsid, None, CLSCTX_SERVER) }?;

    Ok(hr)
}

fn new_update_collection() -> Result<IUpdateCollection, Error> {
    let update_coll_obj = create_object(w!("Microsoft.Update.UpdateColl"))?;

    Ok(update_coll_obj.cast()?)
}

fn wuaupdates(query: impl IntoParam<BSTR>) -> Result<Vec<WUAPackage>, Error> {
    let session = UpdateSession::new()?;

    let updts = session.get_wwuaupdate_collection(query)?;

    let updt_cnt = unsafe { updts.Count() }?;

    let mut packages = vec![];

    if updt_cnt == 0 {
        return Ok(packages);
    }

    for i in 0..updt_cnt {
        let pkg = extract_pkg(&updts, i)?;
        packages.push(pkg);
    }

    Ok(packages)
}

fn extract_pkg(c: &IUpdateCollection, item: i32) -> Result<WUAPackage, Error> {
    let updt = unsafe { c.get_Item(item) }?;

    let identity = unsafe { updt.Identity() }?;

    let (categories, category_ids) = unsafe { categories(&updt) }?;

    let pkg = unsafe {
        WUAPackage {
            title: updt.Title()?.to_string(),
            description: updt.Description()?.to_string(),
            categories,
            category_ids,
            kb_article_ids: kba_ids(&updt)?,
            more_info_urls: more_info_urls(&updt)?,
            support_url: updt.SupportUrl()?.to_string(),
            guid: identity.UpdateID()?.to_string(),
            revision_number: identity.RevisionNumber()?,
            severity: updt.MsrcSeverity()?.to_string(),
            installed: updt.IsInstalled()?.as_bool(),
            downloaded: updt.IsDownloaded()?.as_bool(),
        }
    };

    Ok(pkg)
}

unsafe fn categories(updt: &IUpdate) -> Result<(Vec<String>, Vec<String>), Error> {
    let cat = updt.Categories()?;

    let count = cat.Count()?;

    let (mut cns, mut cids) = (vec![], vec![]);

    if count == 0 {
        return Ok((cns, cids));
    }

    for i in 0..count {
        let item = cat.get_Item(i)?;

        let name = item.Name()?;

        let category_id = item.CategoryID()?;

        cns.push(name.to_string());
        cids.push(category_id.to_string());
    }

    Ok((cns, cids))
}

unsafe fn more_info_urls(updt: &IUpdate) -> Result<Vec<String>, Error> {
    let more_info_urls = updt.MoreInfoUrls()?;

    let count = more_info_urls.Count()?;

    let mut ss = vec![];

    if count == 0 {
        return Ok(ss);
    }

    for i in 0..count {
        let item = more_info_urls.get_Item(i)?;
        ss.push(item.to_string())
    }

    Ok(ss)
}

unsafe fn kba_ids(updt: &IUpdate) -> Result<Vec<String>, Error> {
    let kbarticle_ids = updt.KBArticleIDs()?;

    let count = kbarticle_ids.Count()?;

    let mut ss = vec![];

    if count == 0 {
        return Ok(ss);
    }

    for i in 0..count {
        let item = kbarticle_ids.get_Item(i)?;
        ss.push(item.to_string())
    }

    Ok(ss)
}

pub fn get_win_updates() -> Result<Vec<WUAPackage>, Error> {
    let updates =
        wuaupdates(&"IsInstalled=1 or IsInstalled=0 and Type='Software' and IsHidden=0".into())?;

    #[cfg(debug_assertions)]
    for update in updates.iter() {
        debug!("{update:#?}");
    }

    Ok(updates)
}

#[test]
#[tracing_test::traced_test]
fn test_updates() {
    assert!(get_win_updates().is_ok())
}

pub fn install_updates(guids: Vec<String>) -> Result<bool, Error> {
    let session = UpdateSession::new()?;

    for id in guids {
        let updts = session.get_wwuaupdate_collection(&format!("UpdateID={id}").into())?;

        let updt_cnt = unsafe { updts.Count() }?;

        if updt_cnt == 0 {
            continue;
        }

        for i in 0..updt_cnt {
            let u = unsafe { updts.get_Item(i) }?;
            session.install_wuaupdate(&u)?;
        }
    }

    std::thread::sleep(Duration::from_secs(5));

    let needs_reboot = system_reboot_required_win();

    Ok(needs_reboot)
}

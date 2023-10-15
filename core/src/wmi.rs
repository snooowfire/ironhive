use std::sync::Arc;

use futures_util::StreamExt;
use serde_json::Value;
use wmi::{COMLibrary, WMIConnection};

use crate::error::Error;
use tracing::error;

pub struct WmiManager {
    notify: Arc<tokio::sync::Notify>,
    recv: tokio::sync::broadcast::Receiver<Value>,
}

struct WmiLocalBlocking {
    wmi: WMI,
    sender: tokio::sync::broadcast::Sender<Value>,
    notify: Arc<tokio::sync::Notify>,
}

impl WmiLocalBlocking {
    fn new(oneshot: tokio::sync::oneshot::Sender<WmiManager>) -> Result<Self, Error> {
        let wmi = WMI::new()?;
        let (sender, recv) = tokio::sync::broadcast::channel::<Value>(16);
        let notify = Arc::new(tokio::sync::Notify::const_new());
        let manager = WmiManager {
            notify: notify.clone(),
            recv,
        };
        if oneshot.send(manager).is_err() {
            error!("create manager failed.");
        }
        Ok(Self {
            wmi,
            sender,
            notify,
        })
    }

    async fn run(&self) {
        let Self {
            wmi,
            sender,
            notify,
        } = self;
        loop {
            notify.notified().await;
            if let Err(e) = sender.send(wmi.get_wmi_info().await) {
                error!("send wmi info failed: {e:?}");
            }
        }
    }
}

impl WmiManager {
    pub async fn get_wmi_info(&mut self) -> Result<Value, Error> {
        self.notify.notify_one();
        let info = self.recv.recv().await?;
        Ok(info)
    }

    pub async fn init() -> Result<Self, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        // TODO:
        std::thread::spawn(|| {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let wmi = WmiLocalBlocking::new(tx)?;
                    wmi.run().await;
                    Result::<(), Error>::Ok(())
                })
                .expect("wmi local blocking run failed.");
        });
        let manager = rx.await?;
        Ok(manager)
    }
}

#[tokio::test]
#[tracing_test::traced_test]
async fn test_wmi() {
    if let Ok(mut wmi) = WmiManager::init().await {
        let val = wmi.get_wmi_info().await.unwrap();
        let json = serde_json::to_string_pretty(&val).unwrap();
        println!("{json}");
    }
}

pub struct WMI {
    conn: WMIConnection,
}

impl WMI {
    pub fn new() -> Result<Self, Error> {
        let com_con = COMLibrary::new()?;
        let wmi_con = WMIConnection::new(com_con)?;
        Ok(Self { conn: wmi_con })
    }

    pub async fn test(&self) -> Result<(), Error> {
        let mut vals = Vec::new();
        let mut stream = self
            .conn
            .exec_query_async_native_wrapper("SELECT * FROM Win32_USBController")?;

        while let Some(res) = stream.next().await {
            match res {
                Err(e) => {
                    error!("{e:?}");
                }
                Ok(q) => match serde_json::to_value(q) {
                    Ok(qv) => {
                        vals.push(qv);
                    }
                    Err(se) => {
                        error!("{se:?}");
                    }
                },
            }
        }

        Ok(())
    }

    pub async fn get_wmi_info(&self) -> Value {
        macro_rules! wmi_info {
            ($($name:ident = $value:ident),*) => {{
                $(
                    let $name = self.$value().await;

                    if let Err(ref e) = $name {
                        error!("{e:?}");
                    }

                    let $name = $name.unwrap_or_default();
                )*
                ::serde_json::json!({
                    $(
                        stringify!($name): $name
                    ),*
                })
            }};
        }

        let info = wmi_info!(
            comp_sys_prod = Win32_ComputerSystemProduct,
            comp_sys = Win32_ComputerSystem,
            network_config = Win32_NetworkAdapterConfiguration,
            mem = Win32_PhysicalMemory,
            os = Win32_OperatingSystem,
            base_board = Win32_BaseBoard,
            bios = Win32_BIOS,
            disk = Win32_DiskDrive,
            network_adapter = Win32_NetworkAdapter,
            desktop_monitor = Win32_DesktopMonitor,
            cpu = Win32_Processor,
            usb = Win32_USBController,
            graphics = Win32_VideoController
        );

        info
    }
}

macro_rules! wmi_get_ex {
    ($($name: ident),*) => {
        $(
            #[allow(non_snake_case)]
            async fn $name(&self) -> Result<Vec<::serde_json::Value>,crate::error::Error> {
                let mut vals = Vec::new();
                let ex_stream = self
                .conn
                .exec_query_async_native_wrapper(concat!("SELECT * FROM ",stringify!($name),"EX"));

                if ex_stream.is_ok() {
                    let mut ex_stream = ex_stream.unwrap();
                    while let Some(res) = ex_stream.next().await {
                        match res {
                            Err(e) => {
                                error!("{e:?}");
                            }
                            Ok(q) => match serde_json::to_value(q) {
                                Ok(qv) => {
                                    vals.push(qv);
                                },
                                Err(se) => {
                                    error!("{se:?}");
                                }
                            },
                        }
                    }
                    return Ok(vals);
                }

                let mut stream = self
                    .conn
                    .exec_query_async_native_wrapper(concat!("SELECT * FROM ",stringify!($name)))?;

                while let Some(res) = stream.next().await {
                    match res {
                        Err(e) => {
                            error!("{e:?}");
                        }
                        Ok(q) => match serde_json::to_value(q) {
                            Ok(qv) => {
                                vals.push(qv);
                            },
                            Err(se) => {
                                error!("{se:?}");
                            }
                        },
                    }
                }

                Ok(vals)
            }
        )*
    };
}

macro_rules! wmi_get {
    ($($name: ident),*) => {
        $(
            #[allow(non_snake_case)]
            async fn $name(&self) -> Result<Vec<::serde_json::Value>,crate::error::Error> {
                let mut vals = Vec::new();
                let mut stream = self
                    .conn
                    .exec_query_async_native_wrapper(concat!("SELECT * FROM ",stringify!($name)))?;

                while let Some(res) = stream.next().await {
                    match res {
                        Err(e) => {
                            error!("{e:?}");
                        }
                        Ok(q) => match serde_json::to_value(q) {
                            Ok(qv) => {
                                vals.push(qv);
                            },
                            Err(se) => {
                                error!("{se:?}");
                            }
                        },
                    }
                }

                Ok(vals)
            }
        )*
    };
}

impl WMI {
    wmi_get!(
        Win32_USBController,
        Win32_DesktopMonitor,
        Win32_NetworkAdapter,
        Win32_DiskDrive,
        Win32_ComputerSystemProduct,
        Win32_NetworkAdapterConfiguration,
        Win32_OperatingSystem,
        Win32_BaseBoard,
        Win32_VideoController
    );
    wmi_get_ex!(
        Win32_Processor,
        Win32_BIOS,
        Win32_ComputerSystem,
        Win32_PhysicalMemory
    );
}

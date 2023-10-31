mod agent;
mod checkin;
mod cmd;
mod error;
mod rpc;
mod temp_file;
mod utils;
#[cfg(windows)]
mod windows;

pub use agent::Agent;
pub use error::Error;
pub use rpc::Ironhive;

pub use shared::*;

#[cfg(windows)]
pub use windows::{is_root, ServiceInstaller};

#[cfg(windows)]
pub use windows_service::service::{
    ServiceAction, ServiceActionType, ServiceErrorControl, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceType,
};

pub mod filestore;

use std::io::Error as IoError;
use std::net::{AddrParseError, IpAddr};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io error happened: {0}")]
    IOError(IoError),

    #[error("wrong ip format: {0}")]
    AddrParseError(AddrParseError),
}

pub trait Store {
    fn lock(&self) -> Result<(), StoreError>;
    fn unlock(&self) -> Result<(), StoreError>;
    fn close(&self) -> Result<(), StoreError>;
    fn reserve(
        &self,
        id: &str,
        ifname: &str,
        ip: IpAddr,
        range_id: &str,
    ) -> Result<bool, StoreError>;
    fn last_reserved_ip(&self, range_id: &str) -> Result<IpAddr, StoreError>;
    fn release(&self, ip: IpAddr) -> Result<(), StoreError>;
    fn release_by_id(&self, id: &str, ifname: &str) -> Result<(), StoreError>;
    fn get_by_id(&self, id: &str, ifname: &str) -> Vec<IpAddr>;
}

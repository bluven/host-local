pub mod filestore;

use std::io::Error as IoError;
use std::net::IpAddr;
use thiserror::Error;

#[derive(Debug, Error)]
enum StoreError {
    #[error("io error happened: {0}")]
    IOError(IoError),
}

trait Store {
    fn lock(&self) -> Result<(), StoreError>;
    fn unlock(&self) -> Result<(), StoreError>;
    fn close(&self) -> Result<(), StoreError>;
    fn reserve(id: &str, ifname: &str, ip: IpAddr, range_id: &str) -> Result<bool, StoreError>;
    fn last_reserved_ip(range_id: &str) -> Result<IpAddr, StoreError>;
    fn release(ip: IpAddr) -> Result<(), StoreError>;
    fn release_by_id(id: &str, ifname: &str) -> Result<(), StoreError>;
    fn get_by_id(id: &str, ifname: &str) -> Vec<IpAddr>;
}

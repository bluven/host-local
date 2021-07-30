pub mod range;
pub mod rangeiter;
pub mod rangeset;

use ipnetwork::IpNetwork;
use std::net::IpAddr;

use thiserror::Error;

use super::store::{Store, StoreError};
use rangeiter::RangeIter;
use rangeset::{RangeSet, RangeSetError};

pub struct Allocator {
    range_set: RangeSet,
    store: Box<dyn Store>,
    range_id: String,
}

pub struct IpConfig {
    interface: Option<usize>,
    address: IpNetwork,
    gateway: IpAddr,
}

#[derive(Debug, Error)]
pub enum AllocateError {
    #[error("requested ip {0} is gateway's ip")]
    GatewayIp(IpAddr),

    #[error("{0}")]
    RangeSetError(RangeSetError),

    #[error("{0}")]
    StoreError(StoreError),

    #[error("requested ip {0} is not available")]
    IpNotAvailable(IpAddr),

    #[error("{0} has been allocated to {1}, duplicate allocation is not allowed")]
    DuplicateAllocation(IpAddr, String),

    #[error("ip addresses are exhausted")]
    IpExhausted,
}

impl Allocator {
    pub fn new(range_set: RangeSet, store: Box<dyn Store>, range_id: u32) -> Allocator {
        Allocator {
            range_set: range_set,
            store: store,
            range_id: format!("{}", range_id),
        }
    }

    pub fn get(
        &self,
        id: &str,
        ifname: &str,
        requested_ip: Option<IpAddr>,
    ) -> Result<IpConfig, AllocateError> {
        // todo: store lock

        let mut reserved_ip: IpNetwork;
        let mut gateway: IpAddr;

        match requested_ip {
            Some(ip) => {
                let range = self
                    .range_set
                    .get_range_for_ip(ip)
                    .map_err(AllocateError::RangeSetError)?;

                let reserved = self
                    .store
                    .reserve(id, ifname, ip, &self.range_id)
                    .map_err(AllocateError::StoreError)?;

                if !reserved {
                    return Err(AllocateError::IpNotAvailable(ip));
                }

                reserved_ip = IpNetwork::new(ip, range.subnet.prefix()).unwrap();
            }
            None => {
                let allocated_ips = self.store.get_by_id(id, ifname);
                for ip in allocated_ips.into_iter() {
                    if self.range_set.get_range_for_ip(ip).is_err() {
                        return Err(AllocateError::DuplicateAllocation(ip, id.to_owned()));
                    }
                }

                for (ip_net, gateway) in self.into_iter() {
                    let reserved = self
                        .store
                        .reserve(id, ifname, ip_net.ip(), &self.range_id)
                        .map_err(AllocateError::StoreError)?;

                    if reserved {
                        reserved_ip = ip_net;
                        break;
                    }
                }

                return Err(AllocateError::IpExhausted);
            }
        }

        Ok(IpConfig {
            interface: None,
            address: reserved_ip,
            gateway: gateway,
        })
    }

    pub fn into_iter(&self) -> RangeIter {
        let mut range_iter = RangeIter {
            range_set: self.range_set,
            range_index: 0,
            current_ip: None,
            start_ip: None,
        };

        let mut start_from_last_reserved_ip = false;
        let mut last_reserved_ip: IpAddr;

        if let Ok(ip) = self.store.last_reserved_ip(&self.range_id) {
            last_reserved_ip = ip;
            start_from_last_reserved_ip = self.range_set.contains(last_reserved_ip);
        };

        if start_from_last_reserved_ip {
            for (index, range) in self.range_set.iter().enumerate() {
                if range.contains(last_reserved_ip) {
                    range_iter.range_index = index;
                    range_iter.current_ip = Some(last_reserved_ip);
                    break;
                }
            }
        } else {
            range_iter.range_index = 0;
            range_iter.start_ip = Some(self.range_set.get(0).unwrap().start);
        };

        return range_iter;
    }
}

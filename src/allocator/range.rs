use std::cmp::PartialEq;
use std::net::IpAddr;

use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
  pub subnet: IpNetwork,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub range_start: Option<IpAddr>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub range_end: Option<IpAddr>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub gateway: Option<IpAddr>,
}

#[derive(Debug, Error, PartialEq)]
pub enum RangeError {
  #[error("Network {0} too small to allocate from")]
  TooSmallNetwork(IpNetwork),

  #[error("Network address of subnet {0} should be {1}")]
  WrongNetworkAddr(IpNetwork, IpAddr),
}

impl Range {
  /// Naive implementation of iterating the IP range.
  ///
  /// This iterator will yield every IP available in the range, that is, every
  /// IP in the subnet, except those lower than `range_start`, higher than
  /// `range_end`, or the one which is the `gateway`.
  ///
  /// The current implementation iterates through the entire range and filters
  /// off the excluded IPs as per above. For IPv4 this will likely never be an
  /// issue but IPv6 ranges are monstrous and could spend a long time spinning
  /// before reaching `range_start`.
  pub fn iter_free(&self) -> impl Iterator<Item = IpNetwork> {
    let prefix = self.subnet.prefix();
    let range_start = self.range_start;
    let range_end = self.range_end;
    let gateway = self.gateway;

    self
      .subnet
      .iter()
      .filter(move |ip| {
        if let Some(ref start) = range_start {
          if ip < start {
            // TODO: figure out how to START from there instead
            return false;
          }
        }

        if let Some(ref end) = range_end {
          if ip > end {
            // TODO: figure out how to stop the iterator there instead
            return false;
          }
        }

        if let Some(ref gw) = gateway {
          if ip == gw {
            return false;
          }
        }

        true
      })
      .map(move |ip| (IpNetwork::new(ip, prefix).unwrap()))
    // UNWRAP: panics on invalid prefix, but we got it from another IpNetwork
  }

  pub fn canonicalize(&mut self) -> Result<(), RangeError> {
    use RangeError::*;

    if self.subnet.prefix() > 30 {
      return Err(TooSmallNetwork(self.subnet));
    }

    if self.subnet.ip() != self.subnet.network() {
      return Err(WrongNetworkAddr(self.subnet, self.subnet.network()));
    }

    if self.gateway == None {
      let mut iter = self.subnet.iter();
      let _ = iter.next();
      self.gateway = iter.next();
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::str::FromStr;
  #[test]
  fn range_iter() {
    let range = Range {
      subnet: "10.1.9.32/16".parse().unwrap(),
      range_start: Some(IpAddr::from_str("10.1.0.1").unwrap()),
      range_end: Some(IpAddr::from_str("10.1.0.5").unwrap()),
      gateway: Some(IpAddr::from_str("10.1.0.4").unwrap()),
    };

    let mut iter = range.iter_free();

    assert_eq!(iter.next(), Some("10.1.0.1/16".parse().unwrap()));
    assert_eq!(iter.next(), Some("10.1.0.2/16".parse().unwrap()));
    assert_eq!(iter.next(), Some("10.1.0.3/16".parse().unwrap()));
    assert_eq!(iter.next(), Some("10.1.0.5/16".parse().unwrap()));
  }

  #[test]
  fn range_iter_with_subnet_only() {
    let range = Range {
      subnet: "10.1.9.32/16".parse().unwrap(),
      range_start: None,
      range_end: None,
      gateway: None,
    };

    let mut iter = range.iter_free();

    assert_eq!(iter.next(), Some("10.1.0.0/16".parse().unwrap()))
  }

  #[test]
  fn canonicalize_small_network() {
    let network = "10.1.9.32/31".parse().unwrap();
    let mut range = Range {
      subnet: network,
      range_start: None,
      range_end: None,
      gateway: None,
    };
    assert_eq!(
      range.canonicalize(),
      Err(RangeError::TooSmallNetwork(network))
    );

    let network = "10.1.9.32/32".parse().unwrap();
    let mut range = Range {
      subnet: network,
      range_start: None,
      range_end: None,
      gateway: None,
    };
    assert_eq!(
      range.canonicalize(),
      Err(RangeError::TooSmallNetwork(network))
    );
  }

  #[test]
  fn canonicalize_wrong_network() {
    let network = "2.2.2.1/16".parse().unwrap();
    let mut range = Range {
      subnet: network,
      range_start: None,
      range_end: None,
      gateway: None,
    };

    assert_eq!(
      range.canonicalize(),
      Err(RangeError::WrongNetworkAddr(network, network.network()))
    );
  }

  #[test]
  fn canonicalize_empty_gateway_ip() {
    let mut range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      range_start: None,
      range_end: None,
      gateway: None,
    };
    let _ = range.canonicalize();
    assert_eq!(range.gateway, "2.2.0.1".parse::<IpAddr>().ok());
  }
}

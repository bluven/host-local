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
  pub start: Option<IpAddr>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub end: Option<IpAddr>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub gateway: Option<IpAddr>,
}

#[derive(Debug, Error, PartialEq)]
pub enum RangeError {
  #[error("Network {0} too small to allocate from")]
  TooSmallNetwork(IpNetwork),

  #[error("Network address of subnet {0} should be {1}")]
  WrongNetworkAddr(IpNetwork, IpAddr),

  #[error("IP {1} is out of network {0}")]
  OutOfRangeIp(IpNetwork, IpAddr),
}

impl Range {
  /// Naive implementation of iterating the IP range.
  ///
  /// This iterator will yield every IP available in the range, that is, every
  /// IP in the subnet, except those lower than `start`, higher than
  /// `end`, or the one which is the `gateway`.
  ///
  /// The current implementation iterates through the entire range and filters
  /// off the excluded IPs as per above. For IPv4 this will likely never be an
  /// issue but IPv6 ranges are monstrous and could spend a long time spinning
  /// before reaching `start`.
  pub fn iter_free(&self) -> impl Iterator<Item = IpNetwork> {
    let prefix = self.subnet.prefix();
    let start = self.start;
    let end = self.end;
    let gateway = self.gateway;

    self
      .subnet
      .iter()
      .filter(move |ip| {
        if let Some(ref start) = start {
          if ip < start {
            // TODO: figure out how to START from there instead
            return false;
          }
        }

        if let Some(ref end) = end {
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

  /// canonicalize check all fields and fill start/end/gateway if empty
  pub fn canonicalize(&mut self) -> Result<(), RangeError> {
    use RangeError::*;

    if self.subnet.prefix() > 30 {
      return Err(TooSmallNetwork(self.subnet));
    }

    if self.subnet.ip() != self.subnet.network() {
      return Err(WrongNetworkAddr(self.subnet, self.subnet.network()));
    }

    // todo: out of range check
    if self.gateway == None {
      let mut iter = self.subnet.iter();
      let _ = iter.next();
      self.gateway = iter.next();
    }

    match self.start {
      Some(ip) => {
        if !self.subnet.contains(ip) {
          return Err(RangeError::OutOfRangeIp(self.subnet, ip));
        }
      }
      None => {
        let mut iter = self.subnet.iter();
        let _ = iter.next();
        self.start = iter.next();
      }
    };

    match self.end {
      Some(ip) => {
        if !self.subnet.contains(ip) {
          return Err(RangeError::OutOfRangeIp(self.subnet, ip));
        }
      }
      None => self.end = Some(self.last_ip()),
    };

    Ok(())
  }

  fn last_ip(&self) -> IpAddr {
    match self.subnet {
      IpNetwork::V4(subnet) => {
        let mut octets = subnet.ip().octets();
        let mask = subnet.mask().octets();

        for i in 0..octets.len() {
          octets[i] = octets[i] | (!mask[i])
        }
        octets[3] -= 1;

        IpAddr::from(octets)
      }
      IpNetwork::V6(subnet) => {
        let mut segments = subnet.ip().segments();
        let mask = subnet.mask().segments();

        for i in 0..segments.len() {
          segments[i] = segments[i] | (!mask[i])
        }
        IpAddr::from(segments)
      }
    }
  }

  fn contains(&self, ip: IpAddr) -> bool {
    if !self.subnet.contains(ip) {
      return false;
    }

    if let Some(start) = self.start {
      if start > ip {
        return false;
      }
    }

    if let Some(end) = self.end {
      if end < ip {
        return false;
      }
    }

    return true;
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
      start: Some(IpAddr::from_str("10.1.0.1").unwrap()),
      end: Some(IpAddr::from_str("10.1.0.5").unwrap()),
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
      start: None,
      end: None,
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
      start: None,
      end: None,
      gateway: None,
    };
    assert_eq!(
      range.canonicalize(),
      Err(RangeError::TooSmallNetwork(network))
    );

    let network = "10.1.9.32/32".parse().unwrap();
    let mut range = Range {
      subnet: network,
      start: None,
      end: None,
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
      start: None,
      end: None,
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
      start: None,
      end: None,
      gateway: None,
    };
    let _ = range.canonicalize();
    assert_eq!(range.gateway, "2.2.0.1".parse::<IpAddr>().ok());
  }

  #[test]
  fn canonicalize_start() {
    let start = "2.2.0.1".parse().unwrap();
    let mut range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      start: Some(start),
      end: None,
      gateway: None,
    };

    assert!(range.canonicalize().is_ok());
    assert_eq!(range.start.unwrap(), start);
  }

  #[test]
  fn canonicalize_none_start() {
    let mut range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      start: None,
      end: None,
      gateway: None,
    };

    assert!(range.canonicalize().is_ok());
    assert_eq!(range.start, "2.2.0.1".parse().ok());
  }

  #[test]
  fn canonicalize_out_of_range_start() {
    let start = "2.1.255.255".parse().unwrap();
    let subnet = "2.2.0.0/16".parse().unwrap();

    let mut range = Range {
      subnet: subnet,
      start: Some(start),
      end: None,
      gateway: None,
    };

    assert!(range.canonicalize().is_err());
    assert_eq!(
      range.canonicalize(),
      Err(RangeError::OutOfRangeIp(subnet, start))
    );
  }

  #[test]
  fn canonicalize_end() {
    let start = "2.2.0.1".parse().unwrap();
    let end = "2.2.255.254".parse().unwrap();
    let mut range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      start: Some(start),
      end: Some(end),
      gateway: None,
    };

    assert!(range.canonicalize().is_ok());
    assert_eq!(range.end.unwrap(), end);
  }

  #[test]
  fn canonicalize_end_is_none() {
    let start = "2.2.0.1".parse().unwrap();
    let mut range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      start: Some(start),
      end: None,
      gateway: None,
    };

    assert!(range.canonicalize().is_ok());
    assert_eq!(range.end.unwrap(), "2.2.255.254".parse::<IpAddr>().unwrap());
  }

  #[test]
  fn canonicalize_end_out_of_range() {
    let start = "2.2.0.1".parse().unwrap();
    let end = "2.3.0.1".parse().unwrap();
    let mut range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      start: Some(start),
      end: Some(end),
      gateway: None,
    };

    assert_eq!(
      range.canonicalize(),
      Err(RangeError::OutOfRangeIp(range.subnet, end))
    );
  }

  #[test]
  fn contains_ip() {
    let range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      start: None,
      end: None,
      gateway: None,
    };

    assert!(range.contains("2.2.0.1".parse().unwrap()));
    assert!(range.contains("2.2.255.254".parse().unwrap()));
    assert!(!range.contains("2.1.255.255".parse().unwrap()));
    assert!(!range.contains("2.3.0.0".parse().unwrap()));

    let range = Range {
      subnet: "2.2.0.0/16".parse().unwrap(),
      start: Some("2.2.2.100".parse().unwrap()),
      end: Some("2.2.2.105".parse().unwrap()),
      gateway: None,
    };

    assert!(range.contains("2.2.2.100".parse().unwrap()));
    assert!(range.contains("2.2.2.105".parse().unwrap()));

    assert!(!range.contains("2.2.1.99".parse().unwrap()));
    assert!(!range.contains("2.2.2.106".parse().unwrap()));
  }
}

use std::cmp::PartialEq;
use std::fmt;
use std::net::IpAddr;

use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Range {
  subnet: IpNetwork,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  start: Option<IpAddr>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  end: Option<IpAddr>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  gateway: Option<IpAddr>,
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
  pub fn new(
    subnet: IpNetwork,
    start: Option<IpAddr>,
    end: Option<IpAddr>,
    gateway: Option<IpAddr>,
  ) -> Result<Self, RangeError> {
    let mut range = Range {
      subnet,
      start,
      end,
      gateway,
    };

    match range.canonicalize() {
      Ok(_) => Ok(range),
      Err(err) => Err(err),
    }
  }
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
  fn canonicalize(&mut self) -> Result<(), RangeError> {
    use RangeError::*;

    // todo: ipv6 check
    if self.subnet.is_ipv4() && self.subnet.prefix() > 30 {
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

  // contains checks if a given ip is a valid, allocatable address in a given Range
  pub fn contains(&self, ip: IpAddr) -> bool {
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

  pub fn overlaps(&self, other_range: &Self) -> bool {
    let is_same_familiy = (self.subnet.ip().is_ipv4() && other_range.subnet.ip().is_ipv4())
      || (self.subnet.ip().is_ipv6() && other_range.subnet.ip().is_ipv6());

    if !is_same_familiy {
      return false;
    }

    return self.contains(other_range.start.unwrap())
      || self.contains(other_range.end.unwrap())
      || other_range.contains(self.start.unwrap())
      || other_range.contains(self.end.unwrap());
  }
}

impl fmt::Display for Range {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "({}, {})", self.start.unwrap(), self.end.unwrap())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::str::FromStr;
  #[test]
  fn range_iter() {
    let range = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.1").unwrap()),
      Some(IpAddr::from_str("10.1.0.5").unwrap()),
      Some(IpAddr::from_str("10.1.0.4").unwrap()),
    )
    .unwrap();

    let mut iter = range.iter_free();

    assert_eq!(iter.next(), Some("10.1.0.1/16".parse().unwrap()));
    assert_eq!(iter.next(), Some("10.1.0.2/16".parse().unwrap()));
    assert_eq!(iter.next(), Some("10.1.0.3/16".parse().unwrap()));
    assert_eq!(iter.next(), Some("10.1.0.5/16".parse().unwrap()));
  }

  #[test]
  fn range_iter_with_subnet_only() {
    let range = Range::new("10.1.0.0/16".parse().unwrap(), None, None, None).unwrap();

    let mut iter = range.iter_free();

    assert_eq!(iter.next(), Some("10.1.0.2/16".parse().unwrap()))
  }

  #[test]
  fn canonicalize_small_network() {
    let network = "10.1.0.0/31".parse().unwrap();
    assert_eq!(
      Range::new(network, None, None, None),
      Err(RangeError::TooSmallNetwork(network))
    );

    let network = "10.1.0.0/32".parse().unwrap();
    assert_eq!(
      Range::new(network, None, None, None),
      Err(RangeError::TooSmallNetwork(network))
    );
  }

  #[test]
  fn canonicalize_wrong_network() {
    let network = "2.2.2.1/16".parse().unwrap();

    assert_eq!(
      Range::new(network, None, None, None,),
      Err(RangeError::WrongNetworkAddr(network, network.network()))
    );
  }

  #[test]
  fn canonicalize_empty_gateway_ip() {
    let range = Range::new("2.2.0.0/16".parse().unwrap(), None, None, None).unwrap();
    assert_eq!(range.gateway, "2.2.0.1".parse::<IpAddr>().ok());
  }

  #[test]
  fn canonicalize_start() {
    let start = "2.2.0.1".parse().unwrap();
    let range = Range::new("2.2.0.0/16".parse().unwrap(), Some(start), None, None).unwrap();
    assert_eq!(range.start.unwrap(), start);
  }

  #[test]
  fn canonicalize_none_start() {
    let range = Range::new("2.2.0.0/16".parse().unwrap(), None, None, None).unwrap();
    assert_eq!(range.start, "2.2.0.1".parse().ok());
  }

  #[test]
  fn canonicalize_out_of_range_start() {
    let start = "2.1.255.255".parse().unwrap();
    let subnet = "2.2.0.0/16".parse().unwrap();

    assert_eq!(
      Range::new(subnet, Some(start), None, None,),
      Err(RangeError::OutOfRangeIp(subnet, start))
    );
  }

  #[test]
  fn canonicalize_end() {
    let start = "2.2.0.1".parse().unwrap();
    let end = "2.2.255.254".parse().unwrap();
    let range = Range::new("2.2.0.0/16".parse().unwrap(), Some(start), Some(end), None).unwrap();

    assert_eq!(range.end.unwrap(), end);
  }

  #[test]
  fn canonicalize_end_is_none() {
    let start = "2.2.0.1".parse().unwrap();
    let range = Range::new("2.2.0.0/16".parse().unwrap(), Some(start), None, None).unwrap();

    assert_eq!(range.end.unwrap(), "2.2.255.254".parse::<IpAddr>().unwrap());
  }

  #[test]
  fn canonicalize_end_out_of_range() {
    let start = "2.2.0.1".parse().unwrap();
    let end = "2.3.0.1".parse().unwrap();
    let subnet = "2.2.0.0/16".parse().unwrap();

    assert_eq!(
      Range::new(subnet, Some(start), Some(end), None),
      Err(RangeError::OutOfRangeIp(subnet, end))
    );
  }

  #[test]
  fn contains_ip() {
    let range = Range::new("2.2.0.0/16".parse().unwrap(), None, None, None).unwrap();

    assert!(range.contains("2.2.0.1".parse().unwrap()));
    assert!(range.contains("2.2.255.254".parse().unwrap()));
    assert!(!range.contains("2.1.255.255".parse().unwrap()));
    assert!(!range.contains("2.3.0.0".parse().unwrap()));

    let range = Range::new(
      "2.2.0.0/16".parse().unwrap(),
      Some("2.2.2.100".parse().unwrap()),
      Some("2.2.2.105".parse().unwrap()),
      None,
    )
    .unwrap();

    assert!(range.contains("2.2.2.100".parse().unwrap()));
    assert!(range.contains("2.2.2.105".parse().unwrap()));

    assert!(!range.contains("2.2.1.99".parse().unwrap()));
    assert!(!range.contains("2.2.2.106".parse().unwrap()));
  }

  #[test]
  fn overlaps() {
    let range = Range::new("2.0.0.0/8".parse().unwrap(), None, None, None).unwrap();
    let range2 = Range::new("2.2.0.0/16".parse().unwrap(), None, None, None).unwrap();
    assert!(range.overlaps(&range2));

    let range = Range::new("2.0.0.0/8".parse().unwrap(), None, None, None).unwrap();
    let range2 = Range::new(
      "2001:db8:abcd:0012::0/64".parse().unwrap(),
      None,
      None,
      None,
    )
    .unwrap();
    assert!(!range.overlaps(&range2));

    let range = Range::new("2.2.0.0/16".parse().unwrap(), None, None, None).unwrap();
    let range2 = Range::new("2.3.0.0/16".parse().unwrap(), None, None, None).unwrap();
    assert!(!range.overlaps(&range2));
  }
}

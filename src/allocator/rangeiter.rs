use super::range::Range;
use super::rangeset::RangeSet;
use ipnetwork::IpNetwork;
use num_bigint::{BigInt, Sign};
use std::convert::TryFrom;
use std::net::IpAddr;

pub struct RangeIter {
  pub range_set: RangeSet,
  pub range_index: usize,
  pub current_ip: Option<IpAddr>,
  pub start_ip: Option<IpAddr>,
}

impl Iterator for RangeIter {
  type Item = (IpNetwork, IpAddr);

  fn next(&mut self) -> Option<Self::Item> {
    let range = self.range_set.get(self.range_index);
    if range.is_none() {
      return None;
    }

    let mut range = range.unwrap();

    if self.current_ip.is_none() {
      self.current_ip = Some(range.start);
      self.start_ip = self.current_ip;

      if self.current_ip.unwrap() == range.gateway {
        return self.next();
      }

      let ip_net = IpNetwork::new(self.current_ip.unwrap(), range.subnet.prefix());
      return Some((ip_net.unwrap(), range.gateway));
    }

    if self.current_ip == Some(range.end) {
      self.range_index += 1;
      self.range_index %= self.range_set.len();
      range = self.range_set.get(self.range_index).unwrap();
      self.current_ip = Some(range.start);
    } else {
      self.current_ip = self.current_ip.map(next_ip)
    }

    if self.start_ip.is_none() {
      self.start_ip = self.current_ip
    } else if self.current_ip == self.start_ip {
      return None;
    }

    if self.current_ip.unwrap() == range.gateway {
      return self.next();
    }

    let ip_net = IpNetwork::new(self.current_ip.unwrap(), range.subnet.prefix());
    return Some((ip_net.unwrap(), range.gateway));
  }
}

fn next_ip(ip: IpAddr) -> IpAddr {
  match ip {
    IpAddr::V4(ip) => {
      let octets = ip.octets();

      let bi: BigInt = BigInt::from_bytes_be(Sign::Plus, &octets) + 1;
      let (_, bytes) = bi.to_bytes_be();
      let octets = <[u8; 4]>::try_from(bytes).unwrap();
      IpAddr::from(octets)
    }

    IpAddr::V6(ip) => {
      let octets = ip.octets();
      let bi: BigInt = BigInt::from_bytes_be(Sign::Plus, &octets) + 1;
      let (_, bytes) = bi.to_bytes_be();
      let octets = <[u8; 16]>::try_from(bytes).unwrap();
      IpAddr::from(octets)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::str::FromStr;

  #[test]
  fn iter() {
    let mut ranges = RangeSet::new();

    let r1 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.1").unwrap()),
      Some(IpAddr::from_str("10.1.0.5").unwrap()),
      Some(IpAddr::from_str("10.1.0.4").unwrap()),
    )
    .unwrap();

    let _ = ranges.add(r1);

    let mut ri = RangeIter {
      range_set: ranges,
      range_index: 0,
      current_ip: None,
      start_ip: None,
    };

    let (ip_net, gateway) = ri.next().unwrap();
    assert_eq!(ip_net.ip(), IpAddr::from_str("10.1.0.1").unwrap());
    assert_eq!(ip_net.prefix(), 16u8);
    assert_eq!(gateway, IpAddr::from_str("10.1.0.4").unwrap());

    ri.next();
    ri.next();

    let (ip_net, gateway) = ri.next().unwrap();
    assert_eq!(ip_net.ip(), IpAddr::from_str("10.1.0.5").unwrap());

    assert!(ri.next().is_none());
  }
}

use std::cmp::PartialEq;
use std::fmt;
use std::net::IpAddr;

use thiserror::Error;

use super::range::{Range, RangeError};

pub struct RangeSet {
  ranges: Vec<Range>,
}

#[derive(Debug, Error, PartialEq)]
pub enum RangeSetError {
  #[error("range has different address type")]
  DifferentAddressType,

  #[error("subnet {0} overlaps with subnet {1}")]
  Overlap(Range, Range),

  #[error("no range found for ip {0}")]
  NoRangeForIP(IpAddr),
}

impl RangeSet {
  pub fn new() -> RangeSet {
    RangeSet { ranges: Vec::new() }
  }

  pub fn get_range_for_ip(&self, ip: IpAddr) -> Result<Range, RangeSetError> {
    for r in &self.ranges {
      if r.contains(ip) {
        return Ok(*r);
      }
    }

    return Err(RangeSetError::NoRangeForIP(ip));
  }

  pub fn add(&mut self, range: Range) -> Result<(), RangeSetError> {
    if self.ranges.len() > 0 {
      if !self.ranges[0].is_same_familiy(&range) {
        return Err(RangeSetError::DifferentAddressType);
      }

      for r in &self.ranges {
        if r.overlaps(&range) {
          return Err(RangeSetError::Overlap(*r, range));
        }
      }
    }

    self.ranges.push(range);
    return Ok(());
  }

  pub fn contains(&self, ip: IpAddr) -> bool {
    for range in &self.ranges {
      if range.contains(ip) {
        return true;
      }
    }

    return false;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::str::FromStr;

  #[test]
  fn rangeset_add() {
    let mut ranges = RangeSet::new();

    let r1 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.1").unwrap()),
      Some(IpAddr::from_str("10.1.0.5").unwrap()),
      Some(IpAddr::from_str("10.1.0.4").unwrap()),
    )
    .unwrap();

    assert!(ranges.add(r1).is_ok());

    let r2 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.6").unwrap()),
      Some(IpAddr::from_str("10.1.0.11").unwrap()),
      Some(IpAddr::from_str("10.1.0.7").unwrap()),
    )
    .unwrap();
    assert!(ranges.add(r2).is_ok());

    let r3 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.9").unwrap()),
      Some(IpAddr::from_str("10.1.0.15").unwrap()),
      Some(IpAddr::from_str("10.1.0.15").unwrap()),
    )
    .unwrap();

    assert_eq!(ranges.add(r3), Err(RangeSetError::Overlap(r2, r3)));

    let r4 = Range::new(
      "2001:db8:abcd:0012::0/64".parse().unwrap(),
      None,
      None,
      None,
    )
    .unwrap();
    assert_eq!(ranges.add(r4), Err(RangeSetError::DifferentAddressType));
  }

  #[test]
  fn get_range_for_ip() {
    let mut ranges = RangeSet::new();
    let r1 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.1").unwrap()),
      Some(IpAddr::from_str("10.1.0.5").unwrap()),
      Some(IpAddr::from_str("10.1.0.4").unwrap()),
    )
    .unwrap();

    ranges.add(r1).unwrap();

    let r2 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.6").unwrap()),
      Some(IpAddr::from_str("10.1.0.11").unwrap()),
      Some(IpAddr::from_str("10.1.0.7").unwrap()),
    )
    .unwrap();
    ranges.add(r2).unwrap();

    let ip = "10.1.0.2".parse().unwrap();
    assert_eq!(ranges.get_range_for_ip(ip), Ok(r1));

    let ip = "10.1.0.10".parse().unwrap();
    assert_eq!(ranges.get_range_for_ip(ip), Ok(r2));

    let ip = "10.1.0.12".parse().unwrap();
    assert_eq!(
      ranges.get_range_for_ip(ip),
      Err(RangeSetError::NoRangeForIP(ip))
    );
  }

  #[test]
  fn contains() {
    let mut ranges = RangeSet::new();
    let r1 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.1").unwrap()),
      Some(IpAddr::from_str("10.1.0.5").unwrap()),
      Some(IpAddr::from_str("10.1.0.4").unwrap()),
    )
    .unwrap();

    ranges.add(r1).unwrap();

    let r2 = Range::new(
      "10.1.0.0/16".parse().unwrap(),
      Some(IpAddr::from_str("10.1.0.6").unwrap()),
      Some(IpAddr::from_str("10.1.0.11").unwrap()),
      Some(IpAddr::from_str("10.1.0.7").unwrap()),
    )
    .unwrap();
    ranges.add(r2).unwrap();

    assert!(ranges.contains("10.1.0.2".parse().unwrap()));
    assert!(ranges.contains("10.1.0.10".parse().unwrap()));
    assert!(!ranges.contains("10.1.0.12".parse().unwrap()));
  }
}

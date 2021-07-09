use super::{Store, StoreError};
use std::fs::{create_dir_all, read_to_string, remove_file, OpenOptions};
use std::io::{Error as IoError, ErrorKind, Write};
use std::net::IpAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const LAST_IP_FILE_PREFIX: &str = "last_reserved_ip";
const DEFAULT_DATA_DIR: &str = "/var/lib/cni/networks";
const LINE_BREAK: &str = "\r\n";

#[derive(Debug)]
struct FileStore {
  data_dir: PathBuf,
}

impl FileStore {
  pub fn new(network: &str, data_dir: &str) -> Result<FileStore, StoreError> {
    let mut data_dir = data_dir;
    if data_dir == "" {
      data_dir = DEFAULT_DATA_DIR;
    }

    let path = Path::new(data_dir).join(network);

    create_dir_all(&path)
      .map(|_| FileStore { data_dir: path })
      .map_err(|err| StoreError::IOError(err))
  }

  fn record_last_reserved_ip(&self, ip: IpAddr, range_id: &str) -> Result<(), IoError> {
    let path = self.get_last_reserved_ip_filepath(range_id);

    let mut file = OpenOptions::new()
      .read(true)
      .write(true)
      .create(true)
      .mode(0o644)
      .open(&path)?;

    file.write(ip.to_string().as_bytes()).map(|_| ())
  }

  fn get_last_reserved_ip_filepath(&self, range_id: &str) -> PathBuf {
    self
      .data_dir
      .join(format!("{}.{}", LAST_IP_FILE_PREFIX, range_id))
  }
}

impl Store for FileStore {
  fn lock(&self) -> Result<(), StoreError> {
    return Ok(());
  }

  fn unlock(&self) -> Result<(), StoreError> {
    return Ok(());
  }

  fn close(&self) -> Result<(), StoreError> {
    return Ok(());
  }

  fn reserve(
    &self,
    id: &str,
    ifname: &str,
    ip: IpAddr,
    range_id: &str,
  ) -> Result<bool, StoreError> {
    let fname = self.data_dir.join(ip.to_string());

    let result = OpenOptions::new()
      .read(true)
      .write(true)
      .create(true)
      .create_new(true)
      .open(&fname);

    if let Err(err) = result {
      if err.kind() == ErrorKind::AlreadyExists {
        return Ok(false);
      } else {
        return Err(StoreError::IOError(err));
      }
    }

    let mut content = String::from(id);
    content.push_str(LINE_BREAK);
    content.push_str(ifname);

    let mut file = result.unwrap();

    file
      .write(content.as_bytes())
      .and_then(|_| file.sync_all())
      .map_err(|err| {
        drop(file);
        let _ = remove_file(fname);
        StoreError::IOError(err)
      })?;

    self
      .record_last_reserved_ip(ip, range_id)
      .map(|_| true)
      .map_err(|err| StoreError::IOError(err))
  }

  fn last_reserved_ip(&self, range_id: &str) -> Result<IpAddr, StoreError> {
    let path = self.get_last_reserved_ip_filepath(range_id);

    read_to_string(path)
      .map_err(|err| StoreError::IOError(err))?
      .parse::<IpAddr>()
      .map_err(|err| StoreError::AddrParseError(err))
  }

  fn release(&self, ip: IpAddr) -> Result<(), StoreError> {
    return Ok(());
  }

  fn release_by_id(&self, id: &str, ifname: &str) -> Result<(), StoreError> {
    return Ok(());
  }

  fn get_by_id(&self, id: &str, ifname: &str) -> Vec<IpAddr> {
    return vec![];
  }
}

#[cfg(test)]
mod tests {
  use super::{FileStore, Store, StoreError};
  use std::fs::remove_dir_all;
  use std::io::{Error, ErrorKind};
  use std::net::IpAddr;
  use std::path::Path;

  struct Setup;

  impl Drop for Setup {
    fn drop(&mut self) {
      let _ = remove_dir_all("/tmp/cni");
    }
  }

  #[test]
  fn new() {
    let _ = Setup;

    let result = FileStore::new("test", "/tmp/cni");
    assert!(result.is_ok());
    assert!(Path::new("/tmp/cni/test").exists());

    let result = FileStore::new("test", "");
    assert!(result
      .unwrap_err()
      .to_string()
      .contains("io error happened: "));
  }

  #[test]
  fn reserve() {
    let _ = Setup;

    let cni_data_dir = "/tmp/cni/networks";
    let store = FileStore::new("test", cni_data_dir).unwrap();

    let id = "123456";
    let ifname = "enp2s0";
    let ip = "2.2.2.2".parse::<IpAddr>().unwrap();
    let range_id = "1";

    let result = store.reserve(id, ifname, ip, range_id);
    assert!(result.unwrap(), "{} should be reserved", ip);

    let result = store.reserve(id, ifname, ip, range_id);
    assert!(!result.unwrap(), "{} should not be reserved again", ip);
  }
}

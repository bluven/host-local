use super::{Store, StoreError};
use std::fs::{create_dir_all, read_to_string, remove_file, OpenOptions};
use std::io::{Error as IoError, ErrorKind, Write};
use std::net::IpAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

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
      .map_err(StoreError::IOError)
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
      .map_err(StoreError::IOError)
  }

  fn last_reserved_ip(&self, range_id: &str) -> Result<IpAddr, StoreError> {
    let path = self.get_last_reserved_ip_filepath(range_id);

    read_to_string(path)
      .map_err(StoreError::IOError)?
      .parse::<IpAddr>()
      .map_err(StoreError::AddrParseError)
  }

  fn release(&self, ip: IpAddr) -> Result<(), StoreError> {
    let path = self.data_dir.join(ip.to_string());
    remove_file(path).map_err(StoreError::IOError)
  }

  fn release_by_id(&self, id: &str, ifname: &str) -> Result<(), StoreError> {
    let key = format!("{}{}{}", id, LINE_BREAK, ifname);

    for entry in WalkDir::new(&self.data_dir)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.file_type().is_file())
    {
      let matched = read_to_string(entry.path())
        .map_err(StoreError::IOError)
        .map(|data| data.contains(&key))?;

      if matched {
        remove_file(entry.path()).map_err(StoreError::IOError)?
      }
    }

    Ok(())
  }

  fn get_by_id(&self, id: &str, ifname: &str) -> Vec<IpAddr> {
    let key = format!("{}{}{}", id, LINE_BREAK, ifname);
    let has_key =
      |entry: &DirEntry| read_to_string(entry.path()).map_or(false, |data| data.contains(&key));

    let get_ip_from_path = |entry: DirEntry| {
      entry
        .path()
        .file_name()
        .map(|s| s.to_str())
        .flatten()
        .map(|s| s.parse::<IpAddr>().ok())
        .flatten()
    };

    WalkDir::new(&self.data_dir)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.file_type().is_file())
      .filter(has_key)
      .filter_map(get_ip_from_path)
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::{FileStore, Store, StoreError};
  use std::fs::remove_dir_all;
  use std::io::{Error, ErrorKind};
  use std::net::IpAddr;
  use std::path::Path;

  fn clean_data_dir() {
    let _ = remove_dir_all("/tmp/cni");
  }

  #[test]
  fn new() {
    let result = FileStore::new("test", "/tmp/cni/networks");
    assert!(result.is_ok());
    assert!(Path::new("/tmp/cni/networks/test").exists());

    let result = FileStore::new("test", "");
    assert!(result
      .unwrap_err()
      .to_string()
      .contains("io error happened: "));

    clean_data_dir();
  }

  #[test]
  fn reserve_and_last_reserved_ip() {
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

    let result = store.last_reserved_ip(range_id);
    assert_eq!(result.unwrap(), ip);

    assert!(
      store
        .last_reserved_ip("2")
        .unwrap_err()
        .to_string()
        .contains("io error happened: "),
      "an io error should happened"
    );

    let ips = store.get_by_id(id, ifname);
    assert_eq!(ips.len(), 1);
    assert_eq!(ips[0], ip);

    assert!(store.release(ip).is_ok());
    assert!(!store.data_dir.join(ip.to_string()).exists());

    clean_data_dir();
  }

  #[test]
  fn release_by_id() {
    let cni_data_dir = "/tmp/cni/networks";
    let store = FileStore::new("test", cni_data_dir).unwrap();

    let id = "123456";
    let ifname = "enp2s0";
    let ip = "2.2.2.2".parse::<IpAddr>().unwrap();
    let range_id = "1";

    let result = store.reserve(id, ifname, ip, range_id);
    assert!(result.unwrap(), "{} should be reserved", ip);

    assert!(store.release_by_id(id, ifname).is_ok());
    assert!(!store.data_dir.join(ip.to_string()).exists());

    clean_data_dir();
  }
}

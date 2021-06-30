use super::{Store, StoreError};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

const LAST_IP_FILE_PREFIX: &str = "last_reserved_ip.";
const DEFAULT_DATA_DIR: &str = "/var/lib/cni/networks";
const LINE_BREAK: &str = "\r\n";

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

        match create_dir_all(&path) {
            Ok(_) => Ok(FileStore { data_dir: path }),
            Err(err) => Err(StoreError::IOError(err)),
        }
    }
}

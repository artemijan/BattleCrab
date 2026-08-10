use std::fs::{DirEntry, ReadDir};
use std::iter::FilterMap;
use std::path::{Path, PathBuf};

pub(crate) fn read_dir(
    dir: &Path,
) -> FilterMap<ReadDir, fn(std::io::Result<DirEntry>) -> Option<PathBuf>> {
    match std::fs::read_dir(dir) {
        Ok(read_dir) => read_dir.filter_map(|e| e.ok().map(|e| e.path())),
        Err(_) => panic!("cannot read {}", dir.display()),
    }
}

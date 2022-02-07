use super::{imports::OsStrExt, UdSocketPath};
use std::{ffi::OsStr, fs::remove_file, ops::Drop};

#[derive(Debug)]
pub struct PathDropGuard<'a> {
    pub path: UdSocketPath<'a>,
    pub enabled: bool,
}
impl PathDropGuard<'static> {
    pub fn dummy() -> Self {
        Self {
            path: UdSocketPath::Unnamed,
            enabled: false,
        }
    }
}
impl<'a> Drop for PathDropGuard<'a> {
    fn drop(&mut self) {
        if self.enabled {
            if let UdSocketPath::File(f) = &self.path {
                let path = OsStr::from_bytes(f.to_bytes());
                let _ = remove_file(path);
            }
        }
    }
}

use std::{ffi::OsStr, num::Saturating, os::windows::ffi::OsStrExt};

use crate::NumExt;

pub fn pathcvt<'a>(
    pipe_name: &'a OsStr,
    hostname: Option<&'a OsStr>,
) -> (impl Iterator<Item = &'a OsStr>, usize) {
    const PREFIX_LITERAL: &str = r"\\";
    const PIPEFS_LITERAL: &str = r"\pipe\";
    const LOCAL_HOSTNAME: &str = ".";
    const BASE_LEN: Saturating<usize> = Saturating(PREFIX_LITERAL.len() + PIPEFS_LITERAL.len());

    let hostname = hostname.unwrap_or_else(|| OsStr::new(LOCAL_HOSTNAME));

    let components = [
        OsStr::new(PREFIX_LITERAL),
        hostname,
        OsStr::new(PIPEFS_LITERAL),
        pipe_name,
    ];
    let userlen = hostname.len().saturate() + pipe_name.len().saturate();
    (components.into_iter(), (BASE_LEN + userlen).0)
}
pub fn convert_and_encode_path(pipename: &OsStr, hostname: Option<&OsStr>) -> Vec<u16> {
    let (i, cap) = pathcvt(pipename, hostname);
    let mut path = Vec::with_capacity((cap.saturate() + 1.saturate()).0);
    i.for_each(|c| path.extend(c.encode_wide()));
    path.push(0); // Don't forget the nul terminator!
    path
}
pub fn encode_to_utf16(s: &OsStr) -> Vec<u16> {
    let mut path = s.encode_wide().collect::<Vec<u16>>();
    path.push(0);
    path
}

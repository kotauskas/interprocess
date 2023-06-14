use std::{
    ffi::{OsStr, OsString},
    iter,
    os::windows::ffi::OsStrExt,
};

pub fn pathcvt<'a>(pipe_name: &'a OsStr, hostname: Option<&'a OsStr>) -> (impl Iterator<Item = &'a OsStr>, usize) {
    use iter::once as i;

    static PREFIX_LITERAL: &str = r"\\";
    static PIPEFS_LITERAL: &str = r"\pipe\";
    static LOCAL_HOSTNAME: &str = ".";

    let hostname = hostname.unwrap_or_else(|| OsStr::new(LOCAL_HOSTNAME));

    let iterator = i(OsStr::new(PREFIX_LITERAL))
        .chain(i(hostname))
        .chain(i(OsStr::new(PIPEFS_LITERAL)))
        .chain(i(pipe_name));
    let capacity_hint = PREFIX_LITERAL.len() + hostname.len() + PIPEFS_LITERAL.len() + pipe_name.len();
    (iterator, capacity_hint)
}
pub fn convert_path(pipename: &OsStr, hostname: Option<&OsStr>) -> OsString {
    let (i, cap) = pathcvt(pipename, hostname);
    let mut path = OsString::with_capacity(cap);
    i.for_each(|c| path.push(c));
    path
}
pub fn convert_and_encode_path(pipename: &OsStr, hostname: Option<&OsStr>) -> Vec<u16> {
    let (i, cap) = pathcvt(pipename, hostname);
    let mut path = Vec::with_capacity(cap + 1);
    i.for_each(|c| path.extend(c.encode_wide()));
    path.push(0); // Don't forget the nul terminator!
    path
}
pub fn encode_to_utf16(s: &OsStr) -> Vec<u16> {
    let mut path = s.encode_wide().collect::<Vec<u16>>();
    path.push(0);
    path
}

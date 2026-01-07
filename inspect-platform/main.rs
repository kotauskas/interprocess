#![allow(clippy::exit, clippy::incompatible_msrv)]
use std::ffi::{c_char, c_int, c_long, c_longlong, c_short};

macro_rules! bitwidths {
    ($($nam:ident),+ $(,)?) => {[$((stringify!($nam), $nam::BITS)),+]};
}
#[allow(unused_macros)]
macro_rules! sizes {
    ($($nam:ident),+ $(,)?) => {[$((stringify!($nam), ::std::mem::size_of::<$nam>())),+]};
}

#[cfg(unix)]
mod libc_wrappers;
#[cfg(unix)]
mod unix;

fn maxlen<T>(a: &[(&str, T)]) -> usize { a.iter().map(|&(nm, _)| nm.len()).max().unwrap_or(0) }
fn print_bitwidths(bw: &[(&str, u32)]) {
    let width = maxlen(bw);
    bw.iter().for_each(|&(nm, bw)| println!("{nm:width$} : {bw:>2} bits"));
}
#[allow(dead_code)]
fn print_sizes(sz: &[(&str, usize)]) {
    let width = maxlen(sz);
    sz.iter().for_each(|&(nm, sz)| println!("{nm:width$} : {sz:>3} bytes"));
}
fn print_signedness(nm: &str, signed: bool) {
    println!("{nm} is {}signed", if signed { "" } else { "un" })
}

fn print_common_intro() {
    print_bitwidths(&bitwidths!(usize, c_char, c_short, c_int, c_long, c_longlong));
    print_signedness("c_char", c_char::MIN != 0);
}

fn main() {
    print_common_intro();
    #[cfg(unix)]
    unix::main();
    #[cfg(not(unix))]
    println!("Not a Unix system, no further information will be gathered.");
}

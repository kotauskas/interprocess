use super::{
    super::unixprelude::*,
    c_wrappers,
    cmsg::{read::buf_to_msghdr, CmsgMut, CmsgMutExt, CmsgRef},
    util::{make_msghdr, to_msghdr_iovlen},
    ReadAncillarySuccess, UdSocketPath,
};
use libc::{c_void, iovec, sockaddr_un};
use std::{
    io::{self, IoSlice, IoSliceMut},
    mem::{size_of_val, zeroed},
};

pub(super) fn recvmsg<AB: CmsgMut + ?Sized>(
    fd: BorrowedFd<'_>,
    bufs: &mut [IoSliceMut<'_>],
    ancbuf: &mut AB,
    addrbuf: Option<&mut UdSocketPath<'_>>,
) -> io::Result<ReadAncillarySuccess> {
    let iov = bufs.as_mut_ptr().cast::<iovec>();
    let iovlen = to_msghdr_iovlen(bufs.len())?;
    let mut hdr = make_msghdr(iov, iovlen);
    buf_to_msghdr(ancbuf, &mut hdr)?;

    // SAFETY: sockaddr_un is POD
    let mut addr_buf_staging = unsafe { zeroed::<sockaddr_un>() };
    if addrbuf.is_some() {
        hdr.msg_name = (&mut addr_buf_staging as *mut sockaddr_un).cast::<c_void>();
        #[allow(clippy::useless_conversion)]
        {
            hdr.msg_namelen = size_of_val(&addr_buf_staging).try_into().unwrap();
        }
    }

    let bytes_read = unsafe {
        // SAFETY: make_msghdr_r is good at its job
        c_wrappers::recvmsg(fd, &mut hdr, 0)?
    };

    let advanc = hdr.msg_controllen as _; // FIXME as casts are bad!!
    unsafe {
        // SAFETY: let's hope that recvmsg doesn't just straight up lie to us on the success path
        ancbuf.add_len(advanc);
    }

    if let Some(addr_buf) = addrbuf {
        addr_buf.write_sockaddr_un_to_self(&addr_buf_staging, hdr.msg_namelen as _);
    }

    Ok(ReadAncillarySuccess {
        main: bytes_read,
        ancillary: advanc,
    })
}

pub(super) fn sendmsg(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>], abuf: CmsgRef<'_>) -> io::Result<usize> {
    let iov = bufs.as_ptr().cast_mut().cast::<iovec>();
    let iovlen = to_msghdr_iovlen(bufs.len())?;
    let mut hdr = make_msghdr(iov, iovlen);
    abuf.fill_msghdr(&mut hdr)?;

    unsafe {
        // SAFETY: make_msghdr_w is good at its job
        c_wrappers::sendmsg(fd, &hdr, 0)
    }
}

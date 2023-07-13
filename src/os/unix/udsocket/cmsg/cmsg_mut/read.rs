use super::{super::super::util::to_msghdr_controllen, add_raw::align_first, *};
use libc::{c_void, msghdr};

pub(crate) fn buf_to_msghdr(buf: &mut (impl CmsgMut + ?Sized), hdr: &mut msghdr) -> std::io::Result<()> {
    let ubuf = buf.uninit_part();
    let Some(idx) = align_first(ubuf) else {
        hdr.msg_control = ubuf.as_mut_ptr().cast::<c_void>();
        hdr.msg_controllen = 0;
        return Ok(());
    };
    let subslice = &mut buf.uninit_part()[idx..];
    hdr.msg_control = subslice.as_mut_ptr().cast::<c_void>();
    hdr.msg_controllen = to_msghdr_controllen(subslice.len())?;
    Ok(())
}

use super::{
    super::util::{to_msghdr_controllen, DUMMY_MSGHDR},
    ancillary::{Ancillary, FromCmsg, MalformedPayload, ParseError},
    *,
};
use libc::{c_void, cmsghdr, CMSG_DATA, CMSG_FIRSTHDR, CMSG_NXTHDR};
use std::{cmp::min, io, slice};

/// An immutable reference to a control message buffer that allows for decoding of ancillary data messages.
///
/// The [`decode()`](Self::decode) iterator allows for easy decoding, while [`cmsgs()`](Self::cmsgs) provides low-level access to the raw ancillary message data.
// TODO decoding example
#[derive(Copy, Clone, Debug)]
pub struct CmsgRef<'a>(&'a [u8]);
impl<'a> CmsgRef<'a> {
    /// Creates an empty `CmsgRef`.
    #[inline]
    pub const fn empty() -> Self {
        Self(&[])
    }
    /// Creates a `CmsgRef` from the given byte buffer.
    ///
    /// # Errors
    /// An error is returned if the size of the buffer overflows `isize`.
    ///
    /// # Safety
    /// The contents of `buf` must be valid control messages. Those could be encoded by [`CmsgBuffer`]/[`CmsgMut`] or returned to the program from a system call.
    #[inline]
    pub unsafe fn new_unchecked(buf: &'a [u8]) -> Result<Self, BufferTooBig<&'a [u8], u8>> {
        if buf.len() > isize::MAX as usize {
            return Err(BufferTooBig(buf));
        }
        Ok(Self(buf))
    }

    /// Borrows the buffer, allowing inspection of the underlying data.
    #[inline]
    pub fn inner(&self) -> &[u8] {
        self.0
    }

    /// Returns an iterator over the control messages of the buffer.
    #[inline]
    pub fn cmsgs(self) -> Cmsgs<'a> {
        Cmsgs::new(self)
    }
    /// Returns an iterator that wraps [`cmsgs()`](Self::cmsgs) and decodes them into [`Ancillary`] structs.
    #[inline]
    pub fn decode(self) -> impl Iterator<Item = Result<Ancillary<'a>, ParseError<'a, MalformedPayload>>> {
        self.cmsgs().map(Ancillary::try_parse)
    }

    pub(crate) fn fill_msghdr(&self, hdr: &mut msghdr) -> io::Result<()> {
        hdr.msg_control = self.0.as_ptr().cast::<c_void>().cast_mut();
        hdr.msg_controllen = to_msghdr_controllen(self.0.len())?;
        Ok(())
    }
}

/// Iterator over the control messages in a [`CmsgRef`].
pub struct Cmsgs<'a> {
    buf: CmsgRef<'a>,
    cur: *const cmsghdr,
    dummy: msghdr,
}
impl<'a> Cmsgs<'a> {
    fn new(buf: CmsgRef<'a>) -> Self {
        let mut dummy = DUMMY_MSGHDR;
        buf.fill_msghdr(&mut dummy).unwrap();

        Self {
            buf,
            cur: unsafe {
                // SAFETY: we just constructed the msghdr from a slice
                CMSG_FIRSTHDR(&dummy)
            },
            dummy,
        }
    }
}
impl<'a> Iterator for Cmsgs<'a> {
    type Item = Cmsg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let buf = self.buf.0;
        let one_past_end = buf.as_ptr_range().end;

        if self.cur.is_null() || self.cur.cast::<u8>() >= one_past_end {
            return None;
        }

        let cmsghdr = unsafe {
            // SAFETY: we trust CMSG_FIRSTHDR and CMSG_NXTHDR and have checked for null
            &*self.cur
        };
        let data = unsafe {
            let dptr = CMSG_DATA(cmsghdr);
            if dptr.is_null() {
                return None;
            }

            // SAFETY: we trust CMSG_DATA
            let max_len = one_past_end.offset_from(dptr);
            debug_assert!(max_len >= 0);

            // Buffer overflow check because some OSes (such as everyone's favorite putrid hellspawn macOS) don't
            // even fucking clip the fucking cmsg_len thing to the buffer end as specified by msg_controllen.
            // Source: https://gist.github.com/kentonv/bc7592af98c68ba2738f4436920868dc
            let len = min(cmsghdr.cmsg_len as isize, max_len);

            // SAFETY: we trust CMSG_DATA; the init guarantee comes from CmsgRef containing a slice of initialized data
            slice::from_raw_parts(dptr, len as usize)
        };
        let cmsg = unsafe {
            // SAFETY: as per CmsgRef's safety guarantees
            Cmsg::new(cmsghdr.cmsg_level, cmsghdr.cmsg_type, data)
        };

        self.cur = unsafe {
            // SAFETY: the cursor is being continously fed into CMSG_* pseudomacros from their own output. A null
            // pointer cursor value is handled earlier in the function.
            CMSG_NXTHDR(&self.dummy, self.cur)
        };

        Some(cmsg)
    }
}

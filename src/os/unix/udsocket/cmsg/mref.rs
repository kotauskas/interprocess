use super::{
    super::util::{to_msghdr_controllen, DUMMY_MSGHDR},
    ancillary::{FromCmsg, ParseError},
    *,
};
use libc::{c_void, cmsghdr};
use std::{
    cmp::min,
    io,
    iter::FusedIterator,
    marker::PhantomData,
    slice::{self, SliceIndex},
};

/// An immutable reference to a control message buffer that allows for decoding of ancillary data messages.
///
/// The [`decode()`](Self::decode) iterator allows for easy decoding, while [`cmsgs()`](Self::cmsgs) provides low-level
/// access to the raw ancillary message data.
// TODO decoding example
// TODO to unit struct
#[derive(Copy, Clone, Debug)]
pub struct CmsgRef<'buf>(&'buf [u8]);
impl<'buf> CmsgRef<'buf> {
    /// Creates an empty `CmsgRef`.
    #[inline]
    pub const fn empty() -> Self {
        Self(&[])
    }
    /// Creates a `CmsgRef` from the given byte buffer.
    ///
    /// # Safety
    /// - The contents of `buf` must be valid, well-aligned control messages. Those could be encoded via [`CmsgMut`] or
    /// returned to the program from a system call.
    /// - The length of `buf` must not overflow `isize`.
    #[inline]
    pub unsafe fn new_unchecked(buf: &'buf [u8]) -> Self {
        Self(buf)
    }

    /// Borrows the buffer, allowing inspection of the underlying data.
    #[inline]
    pub fn inner(&self) -> &[u8] {
        self.0
    }

    /// Subslices the buffer to the given range. Inclusive and exclusive, closed, half-open and open ranges may be used
    /// here, as if you were slicing the `[u8]` directly.
    ///
    /// # Safety
    /// The resulting subslice must contain valid ancillary data, i.e. it must be safe to call
    /// [`new_unchecked()`](CmsgRef::new_unchecked) on it.
    pub unsafe fn subslice(&mut self, idx: impl SliceIndex<[u8], Output = [u8]>) {
        self.0 = &self.0[idx];
    }

    /// Cuts off *at least* that many bytes from the beginning of the buffer, or more if the specified amount as an
    /// index does not lie not on a control message boundary.
    pub fn consume_bytes(&mut self, mut amount: usize) {
        if amount == 0 {
            return;
        }
        if amount == self.0.len() {
            self.0 = &[];
            return;
        }

        let mut cmsgs = self.cmsgs();
        while let Some(..) = cmsgs.next() {
            if cmsgs.cur.is_null() {
                return self.consume_bytes(self.0.len());
            }

            let offset = unsafe {
                // SAFETY: CMSG_NXTHDR can only point within the buffer or to null, and we just checked for null
                cmsgs.cur.cast::<u8>().offset_from(self.0.as_ptr())
            };
            debug_assert!(offset >= 0);
            let offset = offset as usize;

            if amount < offset {
                // Jumped over the target index, adjust
                amount = offset;
            }

            if amount == offset {
                unsafe {
                    // SAFETY: we just determined this to be the start of a control message
                    self.subslice(offset..)
                }
                return;
            }
        }

        // If we haven't jumped over or hit the specified amount, it must be somewhere after the beginning of the last
        // control message
        self.consume_bytes(self.0.len())
    }

    /// Returns an iterator over the control messages of the buffer.
    #[inline]
    pub fn cmsgs(&self) -> Cmsgs<'buf> {
        Cmsgs::new(*self)
    }
    /// Returns an iterator that wraps [`cmsgs()`](Self::cmsgs) and decodes them into the ancillary type of your
    /// choosing. (A handy choice is [`Ancillary`](super::ancillary::Ancillary).)
    #[inline]
    pub fn decode<A: FromCmsg<'buf>>(&self) -> Decode<'buf, A> {
        Decode {
            cmsgs: self.cmsgs(),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn fill_msghdr(&self, hdr: &mut msghdr) -> io::Result<()> {
        hdr.msg_control = self.0.as_ptr().cast::<c_void>().cast_mut();
        hdr.msg_controllen = to_msghdr_controllen(self.0.len())?;
        Ok(())
    }
}
impl Default for CmsgRef<'_> {
    #[inline(always)]
    fn default() -> Self {
        Self::empty()
    }
}

/// Iterator over the control messages in a [`CmsgRef`].
///
/// Created by the [`cmsgs()`](CmsgRef::cmsgs) method.
///
/// You probably want to use [`Decode`] instead.
pub struct Cmsgs<'buf> {
    buf: CmsgRef<'buf>,
    cur: *const cmsghdr,
    dummy: msghdr,
}
impl<'buf> Cmsgs<'buf> {
    fn new(buf: CmsgRef<'buf>) -> Self {
        let mut dummy = DUMMY_MSGHDR;
        buf.fill_msghdr(&mut dummy).unwrap();

        Self {
            buf,
            cur: unsafe {
                // SAFETY: we just constructed the msghdr from a slice
                libc::CMSG_FIRSTHDR(&dummy)
            },
            dummy,
        }
    }
}
impl<'buf> Iterator for Cmsgs<'buf> {
    type Item = Cmsg<'buf>;

    #[allow(clippy::unnecessary_cast)]
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
            let dptr = libc::CMSG_DATA(cmsghdr);
            if dptr.is_null() {
                return None;
            }

            // SAFETY: we trust CMSG_DATA
            let max_len = one_past_end.offset_from(dptr);
            debug_assert!(max_len >= 0);

            // cmsg_len includes the size of the cmsghdr and the padding
            let hdrlen = (cmsghdr.cmsg_len - libc::CMSG_LEN(0) as CmsghdrLen) as usize;
            debug_assert!(hdrlen <= isize::MAX as usize);

            // Buffer overflow check because some OSes (such as everyone's favorite putrid hellspawn macOS) don't
            // even fucking clip the fucking cmsg_len thing to the buffer end as specified by msg_controllen.
            // Source: https://gist.github.com/kentonv/bc7592af98c68ba2738f4436920868dc
            let len = min(hdrlen as isize, max_len);

            // SAFETY: we trust CMSG_DATA; the init guarantee comes from CmsgRef containing a slice of initialized data
            slice::from_raw_parts(dptr, len as usize)
        };
        let cmsg = unsafe {
            // SAFETY: as per CmsgRef's safety guarantees
            Cmsg::new(cmsghdr.cmsg_level, cmsghdr.cmsg_type, data)
        };

        self.cur = unsafe {
            // SAFETY: the cursor is being continuously fed into CMSG_* pseudomacros from their own output. A null
            // pointer cursor value is handled earlier in the function.
            libc::CMSG_NXTHDR(&self.dummy, self.cur)
        };

        Some(cmsg)
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
impl<'buf> ExactSizeIterator for Cmsgs<'buf> {
    fn len(&self) -> usize {
        todo!()
    }
}
impl FusedIterator for Cmsgs<'_> {}

/// Iterator that zero-copy deserializes control messages from a [`CmsgRef`].
///
/// Created by the [`decode()`](CmsgRef::decode) method.
pub struct Decode<'buf, A> {
    cmsgs: Cmsgs<'buf>,
    _phantom: PhantomData<fn() -> A>,
}
impl<'buf, A: FromCmsg<'buf>> Iterator for Decode<'buf, A> {
    type Item = Result<A, ParseError<'buf, A::MalformedPayloadError>>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(A::try_parse(self.cmsgs.next()?))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.cmsgs.size_hint()
    }
}
impl<'buf, A: FromCmsg<'buf>> ExactSizeIterator for Decode<'buf, A> {
    #[inline]
    fn len(&self) -> usize {
        self.cmsgs.len()
    }
}
impl<'buf, A: FromCmsg<'buf>> FusedIterator for Decode<'buf, A> {}

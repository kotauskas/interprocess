use super::{
    super::util::{to_msghdr_controllen, DUMMY_MSGHDR},
    ancillary::{FromCmsg, ParseError},
    context::{DummyCollector, DUMMY_COLLECTOR},
    *,
};
use libc::{c_void, cmsghdr, CMSG_DATA, CMSG_FIRSTHDR, CMSG_NXTHDR};
use std::{cmp::min, io, iter::FusedIterator, slice};

/// An immutable reference to a control message buffer that allows for decoding of ancillary data messages.
///
/// The [`decode()`](Self::decode) iterator allows for easy decoding, while [`cmsgs()`](Self::cmsgs) provides low-level access to the raw ancillary message data.
// TODO decoding example
// TODO context
#[derive(Debug)]
pub struct CmsgRef<'b, 'c, C = DummyCollector> {
    buf: &'b [u8],
    /// A borrow of the context collector stored alongside the buffer reference.
    ///
    /// Iteration over the buffer using `Cmsgs` provides access to this field, which is later used when deserializing
    /// them into ancillary data structs.
    pub context_collector: &'c C,
}
impl<'b> CmsgRef<'b, 'static> {
    /// Creates an empty `CmsgRef`.
    #[inline]
    pub const fn empty() -> Self {
        Self {
            buf: &[],
            context_collector: &DUMMY_COLLECTOR,
        }
    }
    /// Creates a `CmsgRef` from the given byte buffer.
    ///
    /// # Safety
    /// - The contents of `buf` must be valid control messages. Those could be encoded by [`CmsgBuffer`]/[`CmsgMut`] or
    /// returned to the program from a system call.
    /// - The length of `buf` must not overflow `isize`.
    #[inline]
    pub unsafe fn new_unchecked(buf: &'b [u8]) -> Self {
        Self {
            buf,
            context_collector: &DUMMY_COLLECTOR,
        }
    }
}
impl<'b, 'c, C> CmsgRef<'b, 'c, C> {
    /// Creates an empty `CmsgRef` with the given context.
    #[inline]
    pub fn empty_with_context(context_collector: &'c C) -> Self {
        Self {
            buf: &[],
            context_collector,
        }
    }
    /// Creates a `CmsgRef` from the given byte buffer and context.
    ///
    /// # Safety
    /// - The contents of `buf` must be valid control messages. Those could be encoded by [`CmsgBuffer`]/[`CmsgMut`] or
    /// returned to the program from a system call.
    /// - The length of `buf` must not overflow `isize`.
    #[inline]
    pub unsafe fn new_unchecked_with_context(buf: &'b [u8], context_collector: &'c C) -> Self {
        Self { buf, context_collector }
    }

    /// Borrows the buffer, allowing inspection of the underlying data.
    #[inline]
    pub fn inner(&self) -> &[u8] {
        self.buf
    }

    /// Returns an iterator over the control messages of the buffer.
    #[inline]
    pub fn cmsgs(&self) -> Cmsgs<'b, 'c, C> {
        Cmsgs::new(*self)
    }
    /// Returns an iterator that wraps [`cmsgs()`](Self::cmsgs) and decodes them into [`Ancillary`] structs.
    #[inline]
    pub fn decode<'slf: 'c, A: FromCmsg<'b, Context = C>>(&'slf self) -> Decode<'b, 'c, A>
    where
        'b: 'c,
    {
        Decode {
            cmsgs: self.cmsgs(),
            context: self.context_collector,
        }
    }

    pub(crate) fn fill_msghdr(&self, hdr: &mut msghdr) -> io::Result<()> {
        hdr.msg_control = self.buf.as_ptr().cast::<c_void>().cast_mut();
        hdr.msg_controllen = to_msghdr_controllen(self.buf.len())?;
        Ok(())
    }
}
impl<C> Copy for CmsgRef<'_, '_, C> {}
impl<C> Clone for CmsgRef<'_, '_, C> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

/// Iterator over the control messages in a [`CmsgRef`].
///
/// Created by the [`cmsgs()`](CmsgRef::cmsgs) method.
///
/// You probably want to use [`Decode`] instead.
pub struct Cmsgs<'b, 'c, C> {
    buf: CmsgRef<'b, 'c, C>,
    cur: *const cmsghdr,
    dummy: msghdr,
}
impl<'b, 'c, C> Cmsgs<'b, 'c, C> {
    fn new(buf: CmsgRef<'b, 'c, C>) -> Self {
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
impl<'b, 'c, C> Iterator for Cmsgs<'b, 'c, C> {
    type Item = Cmsg<'b>;

    fn next(&mut self) -> Option<Self::Item> {
        let buf = self.buf.buf;
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        todo!()
    }
}
impl<'b, 'c, C> ExactSizeIterator for Cmsgs<'b, 'c, C> {
    fn len(&self) -> usize {
        todo!()
    }
}
impl<'b, 'c, C> FusedIterator for Cmsgs<'b, 'c, C> {}

/// Iterator that zero-copy deserializes control messages from a [`CmsgRef`].
///
/// Created by the [`decode()`](CmsgRef::decode) method.
pub struct Decode<'b, 'c, A: FromCmsg<'b>> {
    cmsgs: Cmsgs<'b, 'c, A::Context>,
    context: &'c A::Context,
}
impl<'b, 'c, A: FromCmsg<'b>> Iterator for Decode<'b, 'c, A> {
    type Item = Result<A, ParseError<'b, A::MalformedPayloadError>>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(A::try_parse(self.cmsgs.next()?, self.context))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.cmsgs.size_hint()
    }
}
impl<'b, 'c, A: FromCmsg<'b>> ExactSizeIterator for Decode<'b, 'c, A> {
    #[inline]
    fn len(&self) -> usize {
        self.cmsgs.len()
    }
}
impl<'b, 'c, A: FromCmsg<'b>> FusedIterator for Decode<'b, 'c, A> {}

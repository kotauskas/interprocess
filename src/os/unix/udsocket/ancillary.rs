use super::imports::*;
use cfg_if::cfg_if;
use std::{
    borrow::Cow,
    iter::{FromIterator, FusedIterator},
    mem::size_of,
};

/// Ancillary data to be sent through a Unix domain socket or read from an input buffer.
///
/// Ancillary data gives unique possibilities to Unix domain sockets which no other POSIX API has: passing file descriptors between two processes which do not have a parent-child relationship. It also can be used to transfer credentials of a process reliably.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AncillaryData<'a> {
    /// One or more file descriptors to be sent.
    FileDescriptors(Cow<'a, [c_int]>),
    /// Credentials to be sent. The specified values are checked by the system when sent for all users except for the superuser – for senders, this means that the correct values need to be filled out, otherwise, an error is returned; for receivers, this means that the credentials are to be trusted for authentification purposes. For convenience, the [`credentials`] function provides a value which is known to be valid when sent.
    ///
    /// [`credentials`]: #method.credentials " "
    #[cfg(any(doc, uds_ucred))]
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            all(
                target_os = "linux",
                any(
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "musleabi",
                    target_env = "musleabihf"
                )
            ),
            target_os = "emscripten",
            target_os = "redox"
        )))
    )]
    Credentials {
        /// The process identificator (PID) for the process.
        pid: pid_t,
        /// The user identificator (UID) of the user who started the process.
        uid: uid_t,
        /// The group identificator (GID) of the user who started the process.
        gid: gid_t,
    },
}
impl<'a> AncillaryData<'a> {
    /// The size of a single `AncillaryData::Credentials` element when packed into the Unix ancillary data format. Useful for allocating a buffer when you expect to receive credentials.
    pub const ENCODED_SIZE_OF_CREDENTIALS: usize = Self::_ENCODED_SIZE_OF_CREDENTIALS;
    cfg_if! {
        if #[cfg(uds_ucred)] {
            const _ENCODED_SIZE_OF_CREDENTIALS: usize = size_of::<cmsghdr>() + size_of::<ucred>();
        } /*else if #[cfg(uds_xucred)] {
            const _ENCODED_SIZE_OF_CREDENTIALS: usize = size_of::<cmsghdr>() + size_of::<xucred>();
        } */else if #[cfg(unix)] {
            const _ENCODED_SIZE_OF_CREDENTIALS: usize = size_of::<cmsghdr>();
        } else {
            const _ENCODED_SIZE_OF_CREDENTIALS: usize = 0;
        }
    }

    /// Calculates the size of an `AncillaryData::FileDescriptors` element with the specified amount of file descriptors when packed into the Unix ancillary data format. Useful for allocating a buffer when you expect to receive a specific amount of file descriptors.
    pub const fn encoded_size_of_file_descriptors(num_descriptors: usize) -> usize {
        #[cfg(not(unix))]
        struct cmsghdr; // ???????????????
        size_of::<cmsghdr>() + num_descriptors * size_of::<pid_t>()
    }

    /// Inexpensievly clones `self` by borrowing the `FileDescriptors` variant or copying the `Credentials` variant.
    #[must_use]
    pub fn clone_ref(&'a self) -> Self {
        match *self {
            Self::FileDescriptors(ref fds) => Self::FileDescriptors(Cow::Borrowed(fds)),
            #[cfg(uds_ucred)]
            Self::Credentials { pid, uid, gid } => Self::Credentials { pid, uid, gid },
        }
    }

    /// Returns the size of an ancillary data element when packed into the Unix ancillary data format.
    pub fn encoded_size(&self) -> usize {
        match self {
            Self::FileDescriptors(fds) => Self::encoded_size_of_file_descriptors(fds.len()),
            #[cfg(uds_scm_credentials)]
            Self::Credentials { .. } => Self::ENCODED_SIZE_OF_CREDENTIALS,
        }
    }

    /// Encodes the ancillary data into `EncodedAncillaryData` which is ready to be sent via a Unix domain socket.
    pub fn encode(op: impl IntoIterator<Item = Self>) -> EncodedAncillaryData<'static> {
        let items = op.into_iter();
        let mut buffer = Vec::with_capacity(
            {
                let size_hint = items.size_hint();
                size_hint.1.unwrap_or(size_hint.0)
                // If we assume that all ancillary data elements are credentials, we're more than fine.
            } * Self::ENCODED_SIZE_OF_CREDENTIALS,
        );
        for i in items {
            let mut cmsg_len = size_of::<cmsghdr>();
            let cmsg_level_bytes = SOL_SOCKET.to_ne_bytes();
            let cmsg_type_bytes;

            match i {
                AncillaryData::FileDescriptors(fds) => {
                    cmsg_type_bytes = SCM_RIGHTS.to_ne_bytes();
                    cmsg_len += fds.len() * 4;
                    // #[cfg(target_pointer_width = "64")]
                    // this was here, I don't even remember why, but that
                    // wouldn't compile on a 32-bit machine
                    let cmsg_len_bytes = cmsg_len.to_ne_bytes();
                    buffer.extend_from_slice(&cmsg_len_bytes);
                    buffer.extend_from_slice(&cmsg_level_bytes);
                    buffer.extend_from_slice(&cmsg_type_bytes);
                    for i in fds.iter().copied() {
                        let desc_bytes = i.to_ne_bytes();
                        buffer.extend_from_slice(&desc_bytes);
                    }
                }
                #[cfg(uds_ucred)]
                AncillaryData::Credentials { pid, uid, gid } => {
                    cmsg_type_bytes = SCM_RIGHTS.to_ne_bytes();
                    cmsg_len += size_of::<ucred>();
                    // #[cfg(target_pointer_width = "64")]
                    let cmsg_len_bytes = cmsg_len.to_ne_bytes();
                    let pid_bytes = pid.to_ne_bytes();
                    let uid_bytes = uid.to_ne_bytes();
                    let gid_bytes = gid.to_ne_bytes();
                    buffer.extend_from_slice(&cmsg_len_bytes);
                    buffer.extend_from_slice(&cmsg_level_bytes);
                    buffer.extend_from_slice(&cmsg_type_bytes);
                    buffer.extend_from_slice(&pid_bytes);
                    buffer.extend_from_slice(&uid_bytes);
                    buffer.extend_from_slice(&gid_bytes);
                }
            }
        }
        EncodedAncillaryData(Cow::Owned(buffer))
    }
}
impl AncillaryData<'static> {
    /// Fetches the credentials of the process from the system and returns a value which can be safely sent to another process without the system complaining about an unauthorized attempt to impersonate another process/user/group.
    ///
    /// If you want to send credentials to another process, this is usually the function you need to obtain the desired ancillary payload.
    #[cfg(any(doc, uds_ucred))]
    #[cfg_attr( // uds_ucred template
    feature = "doc_cfg",
        doc(cfg(any(
            all(
                target_os = "linux",
                any(
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "musleabi",
                    target_env = "musleabihf"
                )
            ),
            target_os = "emscripten",
            target_os = "redox"
        )))
    )]
    pub fn credentials() -> Self {
        Self::Credentials {
            pid: unsafe { libc::getpid() },
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
        }
    }
}

/// A buffer used for sending ancillary data into Unix domain sockets.
#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct EncodedAncillaryData<'a>(pub Cow<'a, [u8]>);
impl<'a> From<&'a [u8]> for EncodedAncillaryData<'a> {
    fn from(op: &'a [u8]) -> Self {
        Self(Cow::Borrowed(op))
    }
}
impl From<Vec<u8>> for EncodedAncillaryData<'static> {
    fn from(op: Vec<u8>) -> Self {
        Self(Cow::Owned(op))
    }
}
impl<'b> FromIterator<AncillaryData<'b>> for EncodedAncillaryData<'static> {
    fn from_iter<I: IntoIterator<Item = AncillaryData<'b>>>(iter: I) -> Self {
        AncillaryData::encode(iter)
    }
}
impl<'b> From<Vec<AncillaryData<'b>>> for EncodedAncillaryData<'static> {
    fn from(op: Vec<AncillaryData<'b>>) -> Self {
        Self::from_iter(op)
    }
}
impl<'b: 'c, 'c> From<&'c [AncillaryData<'b>]> for EncodedAncillaryData<'static> {
    fn from(op: &'c [AncillaryData<'b>]) -> Self {
        op.iter().map(AncillaryData::clone_ref).collect::<Self>()
    }
}
impl<'a> AsRef<[u8]> for EncodedAncillaryData<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// A buffer used for receiving ancillary data from Unix domain sockets.
///
/// The actual ancillary data can be obtained using the [`decode`] method.
///
/// # Example
/// See [`UdStream`] or [`UdStreamListener`] for an example of receiving ancillary data.
///
/// [`decode`]: #method.decode " "
/// [`UdStream`]: struct.UdStream.html#examples " "
/// [`UdStreamListener`]: struct.UdStreamListener.html#examples " "
#[derive(Debug)]
pub enum AncillaryDataBuf<'a> {
    /// The buffer's storage is borrowed.
    Borrowed(&'a mut [u8]),
    /// The buffer's storage is owned by the buffer itself.
    Owned(Vec<u8>),
}
impl<'a> AncillaryDataBuf<'a> {
    /// Creates an owned ancillary data buffer with the specified capacity.
    pub fn owned_with_capacity(capacity: usize) -> Self {
        Self::Owned(Vec::with_capacity(capacity))
    }
    /// Creates a decoder which decodes the ancillary data buffer into a friendly representation of its contents.
    ///
    /// All invalid ancillary data blocks are skipped – if there was garbage data in the buffer to begin with, the resulting buffer will either be empty or contain invalid credentials/file descriptors. This should normally never happen if the data is actually received from a Unix domain socket.
    pub fn decode(&'a self) -> AncillaryDataDecoder<'a> {
        AncillaryDataDecoder {
            buffer: self.as_ref(),
            i: 0,
        }
    }
}
impl<'a> From<&'a mut [u8]> for AncillaryDataBuf<'a> {
    fn from(op: &'a mut [u8]) -> Self {
        Self::Borrowed(op)
    }
}
impl From<Vec<u8>> for AncillaryDataBuf<'static> {
    fn from(op: Vec<u8>) -> Self {
        Self::Owned(op)
    }
}
impl<'a> From<&'a mut AncillaryDataBuf<'a>> for AncillaryDataBuf<'a> {
    fn from(op: &'a mut AncillaryDataBuf<'a>) -> Self {
        match op {
            Self::Borrowed(slice) => Self::Borrowed(slice),
            Self::Owned(vec) => Self::Borrowed(vec),
        }
    }
}
impl<'a> AsRef<[u8]> for AncillaryDataBuf<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Borrowed(slice) => slice,
            Self::Owned(vec) => vec,
        }
    }
}
impl<'a> AsMut<[u8]> for AncillaryDataBuf<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Borrowed(slice) => slice,
            Self::Owned(vec) => vec,
        }
    }
}

/// An iterator which decodes ancillary data from an ancillary data buffer.
///
/// This iterator is created by the [`decode`] method on [`AncillaryDataBuf`] – see its documentation for more.
///
/// [`AncillaryDataBuf`]: struct.AncillaryDataBuf.html " "
/// [`decode`]: struct.AncillaryDataBuf.html#method.decode " "
#[derive(Clone, Debug)]
pub struct AncillaryDataDecoder<'a> {
    buffer: &'a [u8],
    i: usize,
}
impl<'a> From<&'a AncillaryDataBuf<'a>> for AncillaryDataDecoder<'a> {
    fn from(buffer: &'a AncillaryDataBuf<'a>) -> Self {
        buffer.decode()
    }
}
impl<'a> Iterator for AncillaryDataDecoder<'a> {
    type Item = AncillaryData<'static>;
    fn next(&mut self) -> Option<Self::Item> {
        fn u32_from_slice(bytes: &[u8]) -> u32 {
            u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        }
        fn u64_from_slice(bytes: &[u8]) -> u64 {
            u64::from_ne_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        }
        let bytes = self.buffer;
        let end = bytes.len() - 1;

        if matches!(bytes.len().checked_sub(self.i), Some(0) | None) {
            self.i = end;
            return None;
        }

        // The first field is the length, which is a size_t
        #[cfg(target_pointer_width = "64")]
        let element_size = {
            if bytes.len() - self.i < 8 {
                self.i = end;
                return None;
            }
            u64_from_slice(&bytes[self.i..self.i + 8]) as usize
        };
        #[cfg(target_pointer_width = "32")]
        let element_size = {
            if bytes.len() - self.i < 4 {
                self.i = end;
                return None;
            }
            u32_from_slice(&bytes[self.i..self.i + 4]) as usize
        };
        // The cmsg_level field is always SOL_SOCKET – we don't need it, let's get the
        // cmsg_type field right away by first getting the offset at which it's
        // located:
        #[cfg(target_pointer_width = "64")]
        let type_offset: usize = 8 + 4; // 8 for cmsg_size, 4 for cmsg_level
        #[cfg(target_pointer_width = "32")]
        let type_offset: usize = 4 + 4; // 4 for cmsg_size, 4 for cmsg_level

        // Now let's get the type itself:
        let element_type = u32_from_slice(&bytes[self.i + type_offset..=self.i + type_offset + 4]);
        // The size of cmsg_size, cmsg_level and cmsg_type together
        let element_offset = type_offset + 4;

        // Update the counter before returning.
        self.i += element_offset // cmsg_size, cmsg_level and cmsg_type
                + element_size; // data size

        // SAFETY: those are ints lmao
        match element_type as i32 {
            SCM_RIGHTS => {
                // We're reading one or multiple descriptors from the ancillary data payload.
                // All descriptors are 4 bytes in size – leftover bytes are discarded thanks
                // to integer division rules
                let amount_of_descriptors = element_size / 4;
                let mut descriptors = Vec::<c_int>::with_capacity(amount_of_descriptors);
                let mut descriptor_offset = element_offset;
                for _ in 0..amount_of_descriptors {
                    descriptors.push(
                        // SAFETY: see above
                        u32_from_slice(&bytes[descriptor_offset..descriptor_offset + 4]) as i32,
                    );
                    descriptor_offset += 4;
                }
                Some(AncillaryData::FileDescriptors(Cow::Owned(descriptors)))
            }
            #[cfg(uds_ucred)]
            SCM_CREDENTIALS => {
                // We're reading a single ucred structure from the ancillary data payload.
                // SAFETY: those are still ints
                let pid_offset = element_offset;
                let pid: pid_t = u32_from_slice(&bytes[pid_offset..pid_offset + 4]) as pid_t;
                let uid_offset = pid_offset + 4;
                let uid: uid_t = u32_from_slice(&bytes[uid_offset..uid_offset + 4]) as uid_t;
                let gid_offset = uid_offset + 4;
                let gid: gid_t = u32_from_slice(&bytes[gid_offset..gid_offset + 4]) as gid_t;
                Some(AncillaryData::Credentials { pid, uid, gid })
            }
            _ => self.next(), // Do nothing if we hit corrupted data.
        }
    }
}
impl FusedIterator for AncillaryDataDecoder<'_> {}

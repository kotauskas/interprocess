use super::PipeModeTag;
use crate::os::windows::winprelude::*;
use std::{convert::TryFrom, mem};
use windows_sys::Win32::{
    Storage::FileSystem::{PIPE_ACCESS_DUPLEX, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND},
    System::Pipes::{PIPE_READMODE_BYTE, PIPE_READMODE_MESSAGE, PIPE_TYPE_BYTE, PIPE_TYPE_MESSAGE},
};

/// The direction of a named pipe connection, designating who can read data and who can write it. This describes the
/// direction of the data flow unambiguously, so that the meaning of the values is the same for the client and server –
/// [`ClientToServer`](PipeDirection::ClientToServer) always means client → server, for example.
#[repr(u32)]
// We depend on the fact that DWORD always maps to u32, which, thankfully, will always stay true
// since the public WinAPI is supposed to be ABI-compatible. Just keep in mind that the
// #[repr(u32)] means that we can transmute this enumeration to the Windows DWORD type.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PipeDirection {
    /// Represents server ← client data flow: clients write data, the server reads it.
    ClientToServer = PIPE_ACCESS_INBOUND,
    /// Represents server → client data flow: the server writes data, clients read it.
    ServerToClient = PIPE_ACCESS_OUTBOUND,
    /// Represents server ⇄ client data flow: the server can write data which then is read by the client, while the
    /// client writes data which is read by the server.
    Duplex = PIPE_ACCESS_DUPLEX,
}
impl PipeDirection {
    /// Returns the role which the pipe client will have in this direction setting.
    ///
    /// # Usage
    /// ```
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeDirection::ClientToServer.client_role(),
    ///     PipeStreamRole::Writer,
    /// );
    /// assert_eq!(
    ///     PipeDirection::ServerToClient.client_role(),
    ///     PipeStreamRole::Reader,
    /// );
    /// assert_eq!(
    ///     PipeDirection::Duplex.client_role(),
    ///     PipeStreamRole::ReaderAndWriter,
    /// );
    /// ```
    pub const fn client_role(self) -> PipeStreamRole {
        match self {
            Self::ClientToServer => PipeStreamRole::Writer,
            Self::ServerToClient => PipeStreamRole::Reader,
            Self::Duplex => PipeStreamRole::ReaderAndWriter,
        }
    }
    /// Returns the role which the pipe server will have in this direction setting.
    ///
    /// # Usage
    /// ```
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeDirection::ClientToServer.server_role(),
    ///     PipeStreamRole::Reader,
    /// );
    /// assert_eq!(
    ///     PipeDirection::ServerToClient.server_role(),
    ///     PipeStreamRole::Writer,
    /// );
    /// assert_eq!(
    ///     PipeDirection::Duplex.server_role(),
    ///     PipeStreamRole::ReaderAndWriter,
    /// );
    /// ```
    pub const fn server_role(self) -> PipeStreamRole {
        match self {
            Self::ClientToServer => PipeStreamRole::Reader,
            Self::ServerToClient => PipeStreamRole::Writer,
            Self::Duplex => PipeStreamRole::ReaderAndWriter,
        }
    }
}
impl TryFrom<u32> for PipeDirection {
    type Error = ();
    /// Converts a Windows constant to a `PipeDirection` if it's in range.
    ///
    /// # Errors
    /// Returns `Err` if the value is not a valid pipe direction constant.
    fn try_from(op: u32) -> Result<Self, ()> {
        assert!((1..=3).contains(&op));
        // See the comment block above for why this is safe.
        unsafe { mem::transmute(op) }
    }
}
impl From<PipeDirection> for u32 {
    fn from(op: PipeDirection) -> Self {
        unsafe { mem::transmute(op) }
    }
}
/// Describes the role of a named pipe stream. In constrast to [`PipeDirection`], the meaning of values here is relative
/// – for example, [`Reader`](PipeStreamRole::Reader) means [`ServerToClient`](PipeDirection::ServerToClient) if you're
/// creating a server and [`ClientToServer`](PipeDirection::ClientToServer) if you're creating a client.
///
/// This enumeration is also not layout-compatible with the `PIPE_ACCESS_*` constants, in contrast to [`PipeDirection`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum PipeStreamRole {
    /// The stream only reads data.
    Reader,
    /// The stream only writes data.
    Writer,
    /// The stream both reads and writes data.
    ReaderAndWriter,
}
impl PipeStreamRole {
    /// Returns the data flow direction of the data stream, assuming that the value describes the role of the server.
    ///
    /// # Usage
    /// ```
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeStreamRole::Reader.direction_as_server(),
    ///     PipeDirection::ClientToServer,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::Writer.direction_as_server(),
    ///     PipeDirection::ServerToClient,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::ReaderAndWriter.direction_as_server(),
    ///     PipeDirection::Duplex,
    /// );
    /// ```
    pub const fn direction_as_server(self) -> PipeDirection {
        match self {
            Self::Reader => PipeDirection::ClientToServer,
            Self::Writer => PipeDirection::ServerToClient,
            Self::ReaderAndWriter => PipeDirection::Duplex,
        }
    }
    /// Returns the data flow direction of the data stream, assuming that the value describes the role of the client.
    ///
    /// # Usage
    /// ```
    /// # use interprocess::os::windows::named_pipe::{PipeDirection, PipeStreamRole};
    /// assert_eq!(
    ///     PipeStreamRole::Reader.direction_as_client(),
    ///     PipeDirection::ServerToClient,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::Writer.direction_as_client(),
    ///     PipeDirection::ClientToServer,
    /// );
    /// assert_eq!(
    ///     PipeStreamRole::ReaderAndWriter.direction_as_client(),
    ///     PipeDirection::Duplex,
    /// );
    /// ```
    pub const fn direction_as_client(self) -> PipeDirection {
        match self {
            Self::Reader => PipeDirection::ServerToClient,
            Self::Writer => PipeDirection::ClientToServer,
            Self::ReaderAndWriter => PipeDirection::Duplex,
        }
    }

    pub(crate) const fn get_for_rm_sm<Rm: PipeModeTag, Sm: PipeModeTag>() -> Self {
        match (Rm::MODE, Sm::MODE) {
            (Some(..), Some(..)) => Self::ReaderAndWriter,
            (Some(..), None) => Self::Reader,
            (None, Some(..)) => Self::Writer,
            (None, None) => unimplemented!(),
        }
    }
}

/// Specifies the mode for a pipe stream.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PipeMode {
    /// Designates that the pipe stream works in byte stream mode, erasing the boundaries of separate messages.
    Bytes = PIPE_TYPE_BYTE,
    /// Designates that the pipe stream works in message stream mode, preserving the boundaries of separate messages yet
    /// still allowing to read them in byte stream mode.
    Messages = PIPE_TYPE_MESSAGE,
}
impl PipeMode {
    /// Converts the value into a raw `u32`-typed constant, either `PIPE_TYPE_BYTE` or `PIPE_TYPE_MESSAGE` depending
    /// on the value.
    pub const fn to_pipe_type(self) -> u32 {
        self as _
    }
    /// Converts the value into a raw `u32`-typed constant, either `PIPE_READMODE_BYTE` or `PIPE_READMODE_MESSAGE`
    /// depending on the value.
    pub const fn to_readmode(self) -> u32 {
        match self {
            Self::Bytes => PIPE_READMODE_BYTE,
            Self::Messages => PIPE_READMODE_MESSAGE,
        }
    }
}
impl TryFrom<u32> for PipeMode {
    type Error = ();
    /// Converts a Windows constant to a `PipeMode` if it's in range. Both `PIPE_TYPE_*` and `PIPE_READMODE_*` are
    /// supported.
    ///
    /// # Errors
    /// Returns `Err` if the value is not a valid pipe stream mode constant.
    fn try_from(op: u32) -> Result<Self, ()> {
        // It's nicer to only match than to check and transmute
        #[allow(unreachable_patterns)] // PIPE_READMODE_BYTE and PIPE_TYPE_BYTE are equal
        match op {
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE => Ok(Self::Bytes),
            PIPE_READMODE_MESSAGE | PIPE_TYPE_MESSAGE => Ok(Self::Messages),
            _ => Err(()),
        }
    }
}

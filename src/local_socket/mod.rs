//! Local sockets, an IPC primitive featuring a server and multiple clients connecting to that server using a filesystem path inside a special namespace, each having a private connection to that server.
//!
//! ## Implementation types
//! Local sockets are not a real IPC method implemented by the OS – they exist to smooth out the difference between two types of underlying implementation: **Unix domain sockets** and **Windows named pipes**. The [`ImplType`] enumeration documents them and provides methods to query whether they are available and their implementation specifics.
//!
//! ### Implementation properties
//! Implementations of the exact same IPC primitive can have subtly different feature sets on different platforms and even on different versions of the same OS. For example, only on Linux and Windows do Ud-sockets support the "anonymous namespace" (and thus feature [`NameTypeSupport::Both`]); on FreeBSD, macOS and the likes, only file paths are available.
//!
//! The [`ImplProperties`] struct, as obtained through [`ImplType`]'s methods, is a source of information on all possible differences between different implementations of local sockets. This is to say that equal [`ImplProperties`] correspond to the same observable behavior of the IPC primitive – if there are any other differences that affect the public API but are not documented by [`ImplProperties`] (besides the mere fact that different IPC primitives use different system APIs), that's a bug in Interprocess!
//!
//! ### Platform-specific namespaces
//! Since only Linux supports putting Ud-sockets in a separate namespace which is isolated from the filesystem, the `LocalSocketName`/`LocalSocketNameBuf` types are used to identify local sockets rather than `OsStr`/`OsString`: on Unix platforms other than Linux, which includes macOS, all flavors of BSD and possibly other Unix-like systems, the only way to name a Ud-socket is to use a filesystem path. As such, those platforms don't have the namespaced socket creation method available. Complicatng matters further, Windows does not support named pipes in the normal filesystem, meaning that namespaced local sockets are the only functional method on Windows.
//!
//! As a way to solve this issue, [`LocalSocketName`]/`LocalSocketNameBuf` only provide creation in a platform-specific way, meaning that crate users are required to query [`NameTypeSupport`] to decide on the socket names.
//!
//! ## Differences from regular sockets
//! A few missing features, primarily on Windows, require local sockets to omit some important functionality, because code relying on it wouldn't be portable. Some notable differences are:
//! - No `.shutdown()` – your communication protocol must manually negotiate end of transmission. Notably, `.read_to_string()` and `.read_all()` will always block indefinitely at some point.
//! - No datagram sockets – the difference in semantics between connectionless datagram Ud-sockets and connection-based named message pipes on Windows does not allow bridging those two into a common API. You can emulate datagrams on top of streams anyway, so no big deal, right?

#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc_cfg", doc(cfg(feature = "tokio")))]
pub mod tokio;

mod listener;
pub use listener::*;

mod stream;
pub use stream::*;

mod name;
pub use name::*;

mod name_type_support;
pub use name_type_support::*;

mod to_name;
pub use to_name::*;

// TODO sync split
// TODO I/O by ref
// TODO extension traits in crate::os for exposing some OS-specific functionality here

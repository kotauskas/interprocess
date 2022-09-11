//! Local sockets, an IPC primitive featuring a server and multiple clients connecting to that server using a filesystem path inside a special namespace, each having a private connection to that server.
//!
//! Local sockets are not a real IPC method implemented by the OS — they were introduced because of the difference between named pipes on Windows and Unix: named pipes on Windows are almost the same as Unix domain sockets on Linux while Unix named pipes (which are referred to as FIFO files in this crate to avoid confusion) are like unnamed pipes but identifiable with a filesystem path: there's no distinction between writers and the first reader takes all. **Simply put, local sockets use named pipes on Windows and Unix domain sockets on Unix.**
//!
//! ## Platform-specific namespaces
//! There's one more problem regarding platform differences: since only Linux supports putting Ud-sockets in a separate namespace which is isolated from the filesystem, the `LocalSocketName`/`LocalSocketNameBuf` types are used to identify local sockets rather than `OsStr`/`OsString`: on Unix platforms other than Linux, which includes macOS, all flavors of BSD and possibly other Unix-like systems, the only way to name a Ud-socket is to use a filesystem path. As such, those platforms don't have the namespaced socket creation method available. Complicatng matters further, Windows does not support named pipes in the normal filesystem, meaning that namespaced local sockets are the only functional method on Windows. As a way to solve this issue, `LocalSocketName`/`LocalSocketNameBuf` only provide creation in a platform-specific way, meaning that crate users are required to use conditional compilation to decide on the socket names.


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

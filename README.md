[![Crates.io](https://img.shields.io/crates/v/interprocess)](https://crates.io/crates/interprocess "Interprocess on Crates.io")
[![Docs.rs](https://img.shields.io/badge/documentation-docs.rs-informational)](https://docs.rs/interprocess "interprocess on Docs.rs")
[![Build Status](https://github.com/kotauskas/interprocess/workflows/Checks%20and%20tests/badge.svg)](https://github.com/kotauskas/interprocess/actions "GitHub Actions page for Interprocess")
![maintenance-status](https://img.shields.io/badge/maintenance-actively%20developed-brightgreen)
[![Rust version: 1.70+](https://img.shields.io/badge/rust%20version-1.70+-orange)][blogpost]

Interprocess communication toolkit for Rust programs. This crate aims to expose as many platform-specific features as
possible while maintaining a uniform interface for all platforms.

## Interprocess communication primitives
Interprocess provides both OS-specific IPC interfaces and cross-platform abstractions for them.

##### Cross-platform IPC APIs
- **Local sockets** – similar to TCP sockets, but use filesystem or namespaced paths instead of ports on
`localhost`, depending on the OS, bypassing the network stack entirely; implemented using named pipes on Windows and
Unix domain sockets on Unix

##### Platform-specific, but present on both Unix-like systems and Windows
- **Unnamed pipes** – anonymous file-like objects for communicating privately in one direction, most commonly used
to communicate between a child process and its parent

##### Unix-only
- **FIFO files** – special type of file which is similar to unnamed pipes but exists on the filesystem, often
referred to as "named pipes" but completely different from Windows named pipes
- **Unix domain sockets** – a type of socket which is built around the standard networking APIs but uses filesystem
paths instead of ports on `localhost`, optionally using a spearate namespace on Linux akin to Windows named pipes

##### Windows-only
- **Named pipes** – closely resemble Unix domain sockets, use a separate namespace instead of on-drive paths

## Asynchronous I/O
Currently, only Tokio for local sockets, Unix domain sockets and Windows named pipes is supported. Support for
`async-std` is planned.

## Platform support
Interprocess supports Windows and all generic Unix-like systems. Additionally, platform-specific extensions are
supported on select systems. The policy with those extensions is to put them behind `#[cfg]` gates and only expose
on the supporting platforms, producing compile errors instead of runtime errors on platforms that have no support
for those features.

Four levels of support (not called *tiers* to prevent confusion with Rust target tiers, since those work completely
differently) are provided by Interprocess. It would be a breaking change for a platform to be demoted, although
promotions quite obviously can happen as minor or patch releases.

##### Explicit support
*OSes at this level: **Windows**, **Linux**, **macOS***

- Interprocess is guaranteed to compile and succeed in running all tests – it would be a critical bug for it not to
- CI, currently provided by GitHub Actions, runs on all of those platforms and displays an ugly red badge if
anything is wrong on any of those systems
- Certain `#[cfg]`-gated platform-specific features are supported with stable public APIs

##### Explicit support without CI
*OSes at this level: **FreeBSD**, **Dragonfly BSD**, **OpenBSD**, **NetBSD***

- Interprocess is expected to compile and succeed in running all tests – it would be a bug for it not to
- Manual testing on local VMs is usually done before every release; CI is not provided solely because GitHub Actions
selfishly ignores the existence of those wonderful systems
- Certain `#[cfg]`-gated platform-specific features are supported with stable public APIs

##### Support by association
*OSes at this level: **Redox**, **Android**, **Fuchsia**, **iOS**, **tvOS**, **watchOS***

- Interprocess is expected to compile and succeed in running all tests – it would be a bug for it not to
- No manual testing is performed, and CI is unavailable because GitHub Actions does not provide it
- Certain `#[cfg]`-gated platform-specific features that originate from other platforms are supported with stable
public APIs because they behave here identically to how they do on an OS with a higher support level

##### Assumed support
*OSes at this level: POSIX-conformant `#[cfg(unix)]` systems not listed above for which the `libc` crate compiles*

- Interprocess is expected to compile and succeed in running all tests – it would be a bug for it not to
- Because this level encompasses a practically infinite amount of systems, no manual testing or CI can exist

## Feature gates
- **`tokio`**, *off* by default – enables support for Tokio-powered efficient asynchronous IPC.

## License
This crate, along with all community contributions made to it, is dual-licensed under [MIT] and
[Apache 2.0].

[MIT license]: https://choosealicense.com/licenses/mit/
[Apache 2.0 license]: https://choosealicense.com/licenses/apache-2.0/
[blogpost]: https://blog.rust-lang.org/2023/06/01/Rust-1.70.0.html

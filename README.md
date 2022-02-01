# Interprocess
[![Crates.io](https://img.shields.io/crates/v/interprocess)](https://crates.io/crates/interprocess "Interprocess on Crates.io")
[![Docs.rs](https://img.shields.io/badge/documentation-docs.rs-informational)](https://docs.rs/interprocess "interprocess on Docs.rs")
[![Build Status](https://github.com/kotauskas/interprocess/workflows/Checks%20and%20tests/badge.svg)](https://github.com/kotauskas/interprocess/actions "GitHub Actions page for Interprocess")

Interprocess communication toolkit for Rust programs. The crate aims to expose as many platform-specific features as possible while maintaining a uniform interface for all platforms.

## Features
- Cross-platform interprocess communication primitives:
    - **Unnamed pipes** — anonymous file-like objects for communicating privately in one direction, most commonly used to communicate between a child process and its parent
    - **Local sockets** — similar to TCP sockets, but use filesystem or namespaced paths instead of ports on `localhost`, depending on the OS, bypassing the network stack entirely; implemented using named pipes on Windows and Unix domain sockets on Unix
- POSIX-specific interprocess communication primitives:
    - **FIFO files** — special type of file which is similar to unnamed pipes but exists on the filesystem, often referred to as "named pipes" but completely different from Windows named pipes
    - **Unix domain sockets** — a type of socket which is built around the standard networking APIs but uses filesystem paths instead of ports on `localhost`, optionally using a spearate namespace on Linux akin to Windows named pipes
    - **POSIX signals** — used to receive short urgent messages from the OS and other programs, as well as sending those messages *(practical usage, other than for compatibility reasons, is strongly discouraged)*
- Windows-specific interprocess communication primitives:
    - **Named pipes** — closely resembles Unix domain sockets, uses a separate namespace instead of on-drive paths
    - **C signals** — like POSIX signals, but with less signal types and a smaller API *(practical usage, other than for compatibility reasons, is strongly discouraged)*
- **Async support** — efficient wrapper around local sockets, Windows named pipes and Ud-sockets for high-performance parallelism, currently only supports the Tokio runtime

## Feature gates
- **`signals`**, *on* by default — enables support for POSIX signals and C signals. Pulls in additional dependencies.
- **`tokio_support`**, *off* by default — enables support for Tokio-powered efficient asynchronous IPC. Cannot simply be named `tokio` because of Cargo limitations.
- **`nonblocking`**, *on* by default — deprecated and will be removed, do not use.

## License
This crate, along with all community contributions made to it, is dual-licensed under the terms of either the [MIT license] or the [Apache 2.0 license].

[MIT license]: https://choosealicense.com/licenses/mit/ " "
[Apache 2.0 license]: https://choosealicense.com/licenses/apache-2.0/ " "

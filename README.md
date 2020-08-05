# Interprocess
[![Crates.io](https://img.shields.io/crates/v/interprocess)](https://crates.io/crates/interprocess "Interprocess on Crates.io")
[![Docs.rs](https://img.shields.io/badge/documentation-docs.rs-informational)](https://docs.rs/interprocess "interprocess on Docs.rs")
[![Build Status](https://github.com/kotauskas/interprocess/workflows/Build/badge.svg)](https://github.com/kotauskas/interprocess/actions "GitHub Actions page for Interprocess")

Interprocess communication toolkit for Rust programs. The crate aims to expose as many platform-specific features as possible while maintaining a uniform interface for all platforms.

## Features
The following interprocess communication primitives are implemented:
- **Unnamed pipes** — anonymous file-like objects for communicating privately in one direction, most commonly used to communicate between a child process and its parent
- **FIFO files** — Unix-specific type of file which is similar to unnamed pipes but exists on the filesystem, often referred to as "named pipes" but completely different from Windows named pipes
- **Unix domain sockets** — Unix-specific socket type which is extremely similar to normal network sockets but uses filesystem paths instead, with the optional Linux feature allowing them to use a spearate namespace akin to Windows named pipes
- **Windows named pipes** — Windows-specific named pipe interface closely resembling Unix domain sockets
- **Local sockets** — platform independent interface utilizing named pipes on Windows and Unix domain sockets on Unix
- **Mailslots** — Windows-specific interprocess communication primitive for short messages, potentially even across the network

[![Crates.io][cratesio-shield]][cratesio] [![Docs.rs][docsrs-shield]][docsrs]
[![Build status][ci-shield]][ci-page] ![Maintenance status][maint-shield]
[![Rust version: 1.75+][msrv-shield]][msrv-blogpost]

[cratesio-shield]: https://img.shields.io/crates/v/interprocess
[docsrs-shield]: https://img.shields.io/badge/documentation-docs.rs-informational
[ci-shield]: https://github.com/kotauskas/interprocess/actions/workflows/checks_and_tests.yml/badge.svg
[maint-shield]: https://img.shields.io/badge/maintenance-actively%20developed-brightgreen
[msrv-shield]: https://img.shields.io/badge/rust%20version-1.75+-orange

[cratesio]: https://crates.io/crates/interprocess "Interprocess on Crates.io"
[docsrs]: https://docs.rs/interprocess "interprocess on Docs.rs"
[ci-page]: https://github.com/kotauskas/interprocess/actions/workflows/checks_and_tests.yml
[msrv-blogpost]: https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html

Interprocess communication toolkit for Rust programs that aims to expose as many platform-specific
features as possible while maintaining a uniform interface for all platforms and encouraging
portable, correct code.

## Communication primitives
Interprocess provides both OS-specific IPC interfaces and cross-platform abstractions for them.

##### Cross-platform IPC APIs
- **Local sockets** – similar to TCP sockets, but use filesystem or namespaced paths instead of
  ports on `localhost`, depending on the OS, bypassing the network stack entirely; implemented
  using named pipes on Windows and Unix domain sockets on Unix

##### Platform-specific, but present on both Unix-like systems and Windows
- **Unnamed pipes** – anonymous file-like objects for communicating privately in one direction,
  most commonly used to communicate between a child process and its parent

##### Unix-only
- **FIFO files** – special type of file which is similar to unnamed pipes but exists on the
  filesystem, often referred to as "named pipes" but completely different from Windows named pipes
- *Unix domain sockets* – Interprocess no longer provides those, as they are present in the
  standard library; they are, however, exposed as local sockets

##### Windows-only
- **Named pipes** – resemble Unix domain sockets, use a separate namespace instead of on-drive
  paths

## Asynchronous I/O
Currently, the only supported async runtime is [Tokio]. Local sockets and Windows named pipes are
provided by Interprocess, while Unix domain sockets are available in Tokio itself.

Support for [`smol`] is planned.

[Tokio]: https://crates.io/crates/tokio
[`smol`]: https://crates.io/crates/smol

## Platform support
Interprocess supports Windows and all generic Unix-like systems. Additionally, platform-specific
extensions are supported on select systems. The policy with those extensions is to put them behind
`#[cfg]` gates and only expose on the supporting platforms, producing compile errors instead of
runtime errors on platforms that have no support for those features.

Four levels of support (not called *tiers* to prevent confusion with Rust target tiers, since
those work completely differently) are provided by Interprocess. It would be a breaking change
for a platform to be demoted, although promotions quite obviously can happen as minor or patch
releases.

##### Explicit support
*OSes at this level: **Windows**, **Linux**, **macOS***

- Interprocess is guaranteed to compile and succeed in running all tests – it would be a critical
  bug for it not to
- CI, currently provided by GitHub Actions, runs on all of those platforms and displays an ugly red
  badge if anything is wrong on any of those systems
- Certain `#[cfg]`-gated platform-specific features are supported with stable public APIs

##### Explicit support with incomplete CI
*OSes at this level: **FreeBSD**, **Android***

- Interprocess is expected to compile and succeed in running all tests – it would be a bug for it
  not to
- GitHub Actions only allows Clippy and Rustdoc to be run for those targets in CI (via
  cross-compilation) due to a lack of native VMs
- Certain `#[cfg]`-gated platform-specific features are supported with stable public APIs

##### Explicit support without CI
*OSes aat this level: **OpenBSD***

- Interprocess is expected to compile and succeed in running all tests – it would be a bug for it
  not to
- Manual testing on local VMs is usually done before every release; no CI happens because those
  targets' standard library `.rlib`s cannot be installed via `rustup target add`
- Certain `#[cfg]`-gated platform-specific features are supported with stable public APIs

##### Support by association
*OSes at this level: **Dragonfly BSD**, **NetBSD**, **Redox**, **Fuchsia**, **iOS**, **tvOS**,
**watchOS***

- Interprocess is expected to compile and succeed in running all tests – it would be a bug for it not to
- No manual testing is performed, and CI is unavailable because GitHub Actions does not provide it
- Certain `#[cfg]`-gated platform-specific features that originate from other platforms are
  supported with stable public APIs because they behave here identically to how they do on an OS with
  a higher support level

##### Assumed support
*OSes at this level: POSIX-conformant `#[cfg(unix)]` systems not listed above for which the `libc`
crate compiles*

- Interprocess is expected to compile and succeed in running all tests – it would be a bug for it
  not to
- Because this level encompasses a practically infinite amount of systems, no manual testing or CI
  can exist

## Feature gates
- **`tokio`**, *off* by default – enables the [Tokio] variants of IPC primitives (where applicable).

## Anti-LLM notice
I (Goat), the author of this software, have never used any LLM (large language model) software
in any context for any reason. I will continue to uphold this in the lack of violent or otherwise
unweatherable external pressure mandating me to do so.

Accordingly, LLMs have not been used in the development of this software, and LLM-generated code
is not present in the source tree. If you desire to avoid software produced with the use of LLMs,
or if you are working under restrictions that prohibit use of LLM-generated dependencies, this
software can safely be added to your allowlist.

Please note that, while an effort is made to avoid introducing dependencies containing
LLM-generated code and ones known to have been made with the use of LLMs by their core developers,
there are insufficient resources to guarantee a total lack of such software in the transitive
dependency tree. If you discover use of LLMs upstream of this software, you are urged to report
this on the issue tracker. The offending dependency will be removed within the constraints of
feasibility and maintainer bandwidth.

Additionally, some dependencies form part of the public API of this software. Those cannot simply
be removed without impacting users and thereby stooping lower than the users of LLMs themselves.
For dependencies that are large and/or part of the public API while not being authored and
maintained by me, an effort is made to gate them behind off-by-default features to minimize the
risk of LLM contamination. Note that this policy predates the LLM pandemic and is also conductive
of proactive prevention of watering hole attacks
([please do not refer to those as "supply-chain attacks"][not-supplier]).

[not-supplier]: https://www.softwaremaxims.com/blog/not-a-supplier

If you would like to see some motivation for this stance, consult the following links:
- [Nearly half of all code generated by so-called AI found to contain security flaws][veracode]
- [AI Incident Database](https://incidentdatabase.ai/)
- [Challenges to ethics of "artificial intelligence" as per Wikipedia](https://en.wikipedia.org/wiki/Ethics_of_artificial_intelligence#Challenges)
- [`curl`: Death by a thousand slops](https://daniel.haxx.se/blog/2025/07/14/death-by-a-thousand-slops/)

[veracode]: https://www.techradar.com/pro/nearly-half-of-all-code-generated-by-ai-found-to-contain-security-flaws-even-big-llms-affected

You might also want to take a visit to the
[LLM Afflicted Software](https://codeberg.org/ai-alternatives/llm-afflicted-software) registry.

Please let me know if you have concise and helpful resources that could be added to the above
list.

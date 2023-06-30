#[cfg(any(uds_ucred, uds_cmsgcred))]
use super::credentials::{Context as CredentialsContext, Credentials};
use super::{
    file_descriptors::FileDescriptors, Cmsg, FromCmsg, ParseError, ParseErrorKind, ParseResult, SizeMismatch, LEVEL,
};
use std::{
    convert::Infallible,
    error::Error,
    fmt::{self, Display, Formatter},
};

/// A dispatch enumeration of all known ancillary message wrapper structs for Ud-sockets.
#[derive(Debug)]
#[non_exhaustive]
#[allow(missing_docs)] // Self-explanatory
pub enum Ancillary<'a> {
    FileDescriptors(FileDescriptors<'a>),
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "emscripten",
            target_os = "redox"
        )))
    )]
    #[cfg(uds_ucred)]
    Credentials(Credentials<'a>),
}
impl<'a> Ancillary<'a> {
    fn parse_fd(cmsg: Cmsg<'a>) -> ParseResult<'a, Self, MalformedPayload> {
        FileDescriptors::try_parse(cmsg, &())
            .map(Self::FileDescriptors)
            .map_err(|e| e.map_payload_err(MalformedPayload::FileDescriptors))
    }
    #[cfg(uds_ucred)]
    fn parse_credentials(cmsg: Cmsg<'a>, ctx: &CredentialsContext) -> ParseResult<'a, Self, MalformedPayload> {
        Credentials::try_parse(cmsg, ctx)
            .map(Self::Credentials)
            .map_err(|e| e.map_payload_err(MalformedPayload::Credentials))
    }
}
impl<'a> FromCmsg<'a> for Ancillary<'a> {
    type MalformedPayloadError = MalformedPayload;
    type Context = Context;
    fn try_parse(cmsg: Cmsg<'a>, ctx: &Context) -> ParseResult<'a, Self, MalformedPayload> {
        let (cml, cmt) = (cmsg.cmsg_level(), cmsg.cmsg_type());
        if cml != LEVEL {
            return Err(ParseError {
                cmsg,
                kind: ParseErrorKind::WrongLevel {
                    expected: Some(LEVEL),
                    got: cml,
                },
            });
        }

        // let's get down to jump tables
        match cmsg.cmsg_type() {
            FileDescriptors::ANCTYPE => Self::parse_fd(cmsg),
            #[cfg(any(uds_ucred, uds_cmsgcred))]
            Credentials::ANCTYPE => Self::parse_credentials(cmsg, &ctx.credentials),
            _ => Err(ParseError {
                cmsg,
                kind: ParseErrorKind::WrongType {
                    expected: None,
                    got: cmt,
                },
            }),
        }
    }
}

/// Compound context collector used by [`Ancillary`]'s [`FromCmsg`] implementation.
///
/// You must collect this context via [`CmsgMut`](super::CmsgMut)'s context collector field to ensure that every type
/// in [`Ancillary`] can be parsed.
#[derive(Debug, Default)]
#[non_exhaustive]
#[allow(missing_docs)]
pub struct Context {
    #[cfg(any(uds_ucred, uds_sockcred))]
    #[cfg_attr(
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "redox",
            target_os = "freebsd",
            target_os = "dragonfly",
        )))
    )]
    pub credentials: super::credentials::Context,
}

/// Compound error type for [`Ancillary`]'s [`FromCmsg`] implementation.
#[derive(Debug)]
#[non_exhaustive]
#[allow(missing_docs)] // Self-explanatory
pub enum MalformedPayload {
    FileDescriptors(SizeMismatch),
    #[cfg_attr( // uds_ucred template
        feature = "doc_cfg",
        doc(cfg(any(
            target_os = "linux",
            target_os = "emscripten",
            target_os = "redox"
        )))
    )]
    #[cfg(uds_ucred)]
    Credentials(SizeMismatch),
}
impl Display for MalformedPayload {
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FileDescriptors(e) => Display::fmt(&e, _f),
            #[cfg(uds_ucred)]
            Self::Credentials(e) => Display::fmt(&e, _f),
        }
    }
}
impl Error for MalformedPayload {}
impl From<Infallible> for MalformedPayload {
    fn from(nuh_uh: Infallible) -> Self {
        match nuh_uh {}
    }
}

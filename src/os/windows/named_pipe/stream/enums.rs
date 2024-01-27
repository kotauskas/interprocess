use super::super::*;

/// Tags for [`PipeStream`]'s generic arguments that specify the directionality of the stream and
/// how it receives and/or sends data (as bytes or as messages).
///
/// This is a sort of const-generic incarnation of the [`PipeMode`] enumeration that allows
/// `PipeStream` to be consolidated into a single struct with generic parameters that decide which
/// traits are implemented on it.
///
/// Some examples of how different `PipeStream` signatures would look:
/// - **`PipeStream<Bytes, Bytes>`** (or, thanks to default generic arguments, simply `PipeStream`)
///   is a duplex stream that receives and sends bytes.
/// - **`PipeStream<Messages, ()>`** is a receive-only message stream.
/// - **`PipeStream<Bytes, Messages>`** is a duplex stream that receives bytes but sends messages.
pub mod pipe_mode {
    use super::*;

    mod seal {
        use super::*;
        pub trait PipeModeTag: Copy + std::fmt::Debug + Eq + Send + Sync + Unpin {
            const MODE: Option<PipeMode>;
        }
        pub trait NotNone: PipeModeTag {}
    }
    pub(crate) use seal::*;

    macro_rules! present_tag {
        ($($(#[$attr:meta])* $tag:ident is $mode:expr),+ $(,)?) => {
            $(
                $( #[$attr] )*
                #[derive(Copy, Clone, Debug, PartialEq, Eq)]
                pub enum $tag {}
                impl PipeModeTag for $tag {
                    const MODE: Option<PipeMode> = $mode;
                }
            )+
        };
    }

    present_tag! {
        /// Tags a direction of a [`PipeStream`] to be absent.
        None is None,
        /// Tags a direction of a [`PipeStream`] to be present with byte-wise semantics.
        Bytes is Some(PipeMode::Bytes),
        /// Tags a direction of a [`PipeStream`] to be present with message-wise semantics.
        Messages is Some(PipeMode::Messages),
    }
    impl NotNone for Bytes {}
    impl NotNone for Messages {}
}
pub(crate) use pipe_mode::{NotNone as PmtNotNone, PipeModeTag};

#![allow(unused_imports)]
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "nonblocking")] {
        pub use blocking::{unblock, Unblock};
        pub use futures::{
            stream::{FusedStream, Stream},
            AsyncRead, AsyncWrite,
        };
    } else {
        pub type Unblock<T> = std::marker::PhantomData<T>;
    }
}

macro_rules! forward_debug {
    ($ty:ident) => {
        impl ::std::fmt::Debug for $ty {
            #[inline(always)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
    };
}

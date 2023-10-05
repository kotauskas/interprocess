use std::fmt::{self, Debug, Formatter};
pub(crate) fn debug_forward_with_custom_name(nm: &str, fld: &dyn Debug, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_tuple(nm).field(fld).finish()
}

macro_rules! forward_debug {
    ($ty:ident, $nm:literal) => {
        impl ::std::fmt::Debug for $ty {
            #[inline(always)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                $crate::macros::debug_forward_with_custom_name($nm, &self.0, f)
            }
        }
    };
    ($ty:ident) => {
        impl ::std::fmt::Debug for $ty {
            #[inline(always)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
    };
}

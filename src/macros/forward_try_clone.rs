macro_rules! forward_try_clone {
    ($ty:ident $(<$($lt:lifetime),+ $(,)?>)?) => {
        impl crate::TryClone for $ty $(<$($lt),+>)? {
            #[inline]
            fn try_clone(&self) -> ::std::io::Result<Self> {
                Ok(Self(crate::TryClone::try_clone(&self.0)?))
            }
        }
    };
}

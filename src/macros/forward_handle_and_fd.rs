//! Forwarding macros that implement safe handle manipulation in terms of a field's implementations.
//! Usually followed up by one of the derives from `derive_raw`.

macro_rules! forward_as_handle {
	(@impl $({$($lt:tt)*})? $ty:ty, $hty:ident, $trt:ident, $mtd:ident, $cfg:ident) => {
		#[cfg($cfg)]
		impl $(<$($lt)*>)? ::std::os::$cfg::io::$trt for $ty {
			#[inline]
			fn $mtd(&self) -> ::std::os::$cfg::io::$hty<'_> {
				::std::os::$cfg::io::$trt::$mtd(&self.0)
			}
		}
	};
	($({$($lt:tt)*})? $ty:ty, windows) => {
		forward_as_handle!(@impl $({$($lt)*})? $ty, BorrowedHandle, AsHandle, as_handle, windows);
	};
	($({$($lt:tt)*})? $ty:ty, unix) => {
		forward_as_handle!(@impl $({$($lt)*})? $ty, BorrowedFd, AsFd, as_fd, unix);
	};
	($({$($lt:tt)*})? $ty:ty) => {
		forward_as_handle!($({$($lt)*})? $ty, windows);
		forward_as_handle!($({$($lt)*})? $ty, unix);
	};
}

macro_rules! forward_into_handle {
	(@impl $({$($lt:tt)*})? $ty:ty, $hty:ident, $cfg:ident) => {
		#[cfg($cfg)]
		impl $(<$($lt)*>)? ::std::convert::From<$ty> for ::std::os::$cfg::io::$hty {
			#[inline]
			fn from(x: $ty) -> Self {
				::std::convert::From::from(x.0)
			}
		}
	};
	($({$($lt:tt)*})? $ty:ty, windows) => {
		forward_into_handle!(@impl $({$($lt)*})? $ty, OwnedHandle, windows);
	};
	($({$($lt:tt)*})? $ty:ty, unix) => {
		forward_into_handle!(@impl $({$($lt)*})? $ty, OwnedFd, unix);
	};
	($({$($lt:tt)*})? $ty:ty) => {
		forward_into_handle!($({$($lt)*})? $ty, windows);
		forward_into_handle!($({$($lt)*})? $ty, unix);
	};
}

macro_rules! forward_from_handle {
	(@impl $({$($lt:tt)*})? $ty:ty, $hty:ident, $cfg:ident) => {
		#[cfg($cfg)]
		impl $(<$($lt)*>)? ::std::convert::From<::std::os::$cfg::io::$hty> for $ty {
			#[inline]
			fn from(x: ::std::os::$cfg::io::$hty) -> Self {
				Self(::std::convert::From::from(x))
			}
		}
	};
	($({$($lt:tt)*})? $ty:ty, windows) => {
		forward_from_handle!(@impl $({$($lt)*})? $ty, OwnedHandle, windows);
	};
	($({$($lt:tt)*})? $ty:ty, unix) => {
		forward_from_handle!(@impl $({$($lt)*})? $ty, OwnedFd, unix);
	};
	($({$($lt:tt)*})? $ty:ty) => {
		forward_from_handle!($({$($lt)*})? $ty, windows);
		forward_from_handle!($({$($lt)*})? $ty, unix);
	};
}

macro_rules! forward_asinto_handle {
	($({$($lt:tt)*})? $ty:ty, windows) => {
		forward_as_handle!($({$($lt)*})? $ty, windows);
		forward_into_handle!($({$($lt)*})? $ty, windows);
	};
	($({$($lt:tt)*})? $ty:ty, unix) => {
		forward_as_handle!($({$($lt)*})? $ty, unix);
		forward_into_handle!($({$($lt)*})? $ty, unix);
	};
	($({$($lt:tt)*})? $ty:ty) => {
		forward_asinto_handle!($({$($lt)*})? $ty, windows);
		forward_asinto_handle!($({$($lt)*})? $ty, unix);
	};
}

macro_rules! forward_handle {
	($({$($lt:tt)*})? $ty:ty, windows) => {
		forward_asinto_handle!($({$($lt)*})? $ty, windows);
		forward_from_handle!($({$($lt)*})? $ty, windows);
	};
	($({$($lt:tt)*})? $ty:ty, unix) => {
		forward_asinto_handle!($({$($lt)*})? $ty, unix);
		forward_from_handle!($({$($lt)*})? $ty, unix);
	};
	($({$($lt:tt)*})? $ty:ty) => {
		forward_handle!($({$($lt)*})? $ty, windows);
		forward_handle!($({$($lt)*})? $ty, unix);
	};
}

// TODO deal with this documentation mess (also allow replacing that Error signature with a name)
macro_rules! forward_try_into_handle {
	(@impl $({$($lt:tt)*})? $ty:ty, $fldt:path, $hty:ident, $cfg:ident) => {
		/// Releases ownership of the handle/file descriptor, detaches the object from the async
		/// runtime and returns the handle/file descriptor as an owned object.
		///
		/// # Errors
		/// If called outside the async runtime that corresponds to this type.
		#[cfg($cfg)]
		impl $(<$($lt)*>)? ::std::convert::TryFrom<$ty> for ::std::os::$cfg::io::$hty {
			type Error = <::std::os::$cfg::io::$hty as ::std::convert::TryFrom<$fldt>>::Error;
			#[inline]
			fn try_from(x: $ty) -> Result<Self, Self::Error> {
				::std::convert::TryFrom::try_from(x.0)
			}
		}
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path, windows) => {
		forward_try_into_handle!(@impl $({$($lt)*})? $ty, $fldt, OwnedHandle, windows);
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path, unix) => {
		forward_try_into_handle!(@impl $({$($lt)*})? $ty, $fldt, OwnedFd, unix);
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path) => {
		forward_try_into_handle!($({$($lt)*})? $ty, windows);
		forward_try_into_handle!($({$($lt)*})? $ty, unix);
	};
}

macro_rules! forward_try_from_handle {
	(@impl $({$($lt:tt)*})? $ty:ty, $fldt:path, $hty:ident, $cfg:ident) => {
		/// Creates an async object from a given owned handle/file descriptor. This will also attach
		/// the object to the async runtime this function is called in.
		///
		/// # Errors
		/// If called outside the async runtime that corresponds to this type.
		#[cfg($cfg)]
		impl $(<$($lt)*>)? ::std::convert::TryFrom<::std::os::$cfg::io::$hty> for $ty {
			type Error = <$fldt as ::std::convert::TryFrom<::std::os::$cfg::io::$hty>>::Error;
			#[inline]
			fn try_from(x: ::std::os::$cfg::io::$hty) -> Result<Self, Self::Error> {
				Ok(Self(::std::convert::TryFrom::try_from(x)?))
			}
		}
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path, windows) => {
		forward_try_from_handle!(@impl $({$($lt)*})? $ty, $fldt, OwnedHandle, windows);
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path, unix) => {
		forward_try_from_handle!(@impl $({$($lt)*})? $ty, $fldt, OwnedFd, unix);
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path) => {
		forward_try_from_handle!($({$($lt)*})? $ty, $fldt, windows);
		forward_try_from_handle!($({$($lt)*})? $ty, $fldt, unix);
	};
}

macro_rules! forward_try_handle {
	($({$($lt:tt)*})? $ty:ty, $fldt:path, windows) => {
		forward_try_into_handle!($({$($lt)*})? $ty, $fldt, windows);
		forward_try_from_handle!($({$($lt)*})? $ty, $fldt, windows);
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path, unix) => {
		forward_try_into_handle!($({$($lt)*})? $ty, $fldt, unix);
		forward_try_from_handle!($({$($lt)*})? $ty, $fldt, unix);
	};
	($({$($lt:tt)*})? $ty:ty, $fldt:path) => {
		forward_try_handle!($({$($lt)*})? $ty, $fldt, windows);
		forward_try_handle!($({$($lt)*})? $ty, $fldt, unix);
	};
}

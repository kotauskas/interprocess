/// Only dispatches with &self and &mut self.
macro_rules! dispatch {
	($ty:ident: $nm:ident in $var:expr => $e:expr) => {{
		match $var {
			#[cfg(windows)]
			$ty::NamedPipe(_arm) => {
				let mut _arm2 = _arm;
				let $nm = &mut _arm2;
				$e
			}
			#[cfg(unix)]
			$ty::UdSocket(_arm) => {
				let mut _arm2 = _arm;
				let $nm = &mut _arm2;
				$e
			}
		}
	}};
}

macro_rules! dispatch_asraw {
	($ty:ident) => {
		#[cfg(windows)]
		impl AsHandle for $ty {
			#[inline]
			fn as_handle(&self) -> BorrowedHandle<'_> {
				dispatch!(Self: x in self => (*x).as_handle())
			}
		}
	};
}

macro_rules! mkenum {
	($(#[$($attr:tt)+])* $nm:ident) => {
		$(#[$($attr)+])*
		#[derive(Debug)]
		pub enum $nm {
			/// Makes use of Windows named pipes.
			///
			/// Click the struct name in the parentheses to learn more.
			#[cfg(windows)]
			#[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
			NamedPipe(np_impl::$nm),
			/// Makes use of Unix domain sockets.
			///
			/// Click the struct name in the parentheses to learn more.
			#[cfg(unix)]
			#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
			UdSocket(uds_impl::$nm),
		}
		// TODO is doc(hidden) still necessary?
		impl $crate::Sealed for $nm {}
		#[cfg(windows)]
		#[cfg_attr(feature = "doc_cfg", doc(cfg(windows)))]
		impl From<np_impl::$nm> for $nm {
			fn from(x: np_impl::$nm) -> Self {
				Self::NamedPipe(x)
			}
		}
		#[cfg(unix)]
		#[cfg_attr(feature = "doc_cfg", doc(cfg(unix)))]
		impl From<uds_impl::$nm> for $nm {
			fn from(x: uds_impl::$nm) -> Self {
				Self::UdSocket(x)
			}
		}
	};
}

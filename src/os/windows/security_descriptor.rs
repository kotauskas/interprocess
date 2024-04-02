mod as_security_descriptor;
mod borrowed;
mod c_wrappers;
mod ext;
mod owned;
mod try_clone;

#[allow(unused_imports)] // this is literally a false positive
pub(crate) use try_clone::LocalBox;

pub use {as_security_descriptor::*, borrowed::*, ext::*, owned::*};

use try_clone::clone;

use std::{ffi::c_void, io};
use windows_sys::Win32::Security::{IsValidSecurityDescriptor, SECURITY_ATTRIBUTES};

// TODO maybe make public and remove reexport

unsafe fn validate(ptr: *mut c_void) {
	unsafe {
		debug_assert!(
			IsValidSecurityDescriptor(ptr) == 1,
			"invalid security descriptor: {}",
			io::Error::last_os_error(),
		);
	}
}

pub(super) fn create_security_attributes(
	sd: Option<BorrowedSecurityDescriptor<'_>>,
	inheritable: bool,
) -> SECURITY_ATTRIBUTES {
	let mut attrs = unsafe { std::mem::zeroed::<SECURITY_ATTRIBUTES>() };
	if let Some(sd) = sd {
		sd.write_to_security_attributes(&mut attrs);
	}
	attrs.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
	attrs.bInheritHandle = inheritable as i32;
	attrs
}

use std::{borrow::Borrow, ffi::c_void, fmt::Debug};
use windows_sys::Win32::Security::{
	IsValidSecurityDescriptor, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
};

/// A borrowed [security descriptor][sd] which is known to be safe to use.
///
/// [sd]: https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-security_descriptor
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct SecurityDescriptor(SECURITY_DESCRIPTOR);
impl SecurityDescriptor {
	/// Borrows the given security descriptor.
	///
	/// # Safety
	/// -	The `SECURITY_DESCRIPTOR` structure includes pointer fields which Windows later
	/// 	dereferences. Having those pointers point to garbage, uninitialized memory or
	/// 	non-dereferencable regions constitutes undefined behavior.
	/// -	The pointers contained inside must not be aliased by mutable references.
	/// -	`IsValidSecurityDescriptor()` must return `true` for the given value.
	#[inline]
	pub unsafe fn from_ref(r: &SECURITY_DESCRIPTOR) -> &Self {
		unsafe {
			let ret = std::mem::transmute::<_, &Self>(r);
			debug_assert!(
				IsValidSecurityDescriptor(ret.as_ptr()) == 1,
				"invalid security descriptor"
			);
			ret
		}
	}
	/// Casts to the `void*` type seen in `SECURITY_ATTRIBUTES`.
	#[inline]
	pub fn as_ptr(&self) -> *mut c_void {
		(self as *const Self).cast_mut().cast()
	}
	/// Sets the security descriptor pointer of the given `SECURITY_ATTRIBUTES` structure to the
	/// security descriptor borrow of `self`.
	pub fn write_to_security_attributes(&self, attributes: &mut SECURITY_ATTRIBUTES) {
		attributes.lpSecurityDescriptor = self.as_ptr();
	}

	pub(super) fn create_security_attributes(
		slf: Option<&Self>,
		inheritable: bool,
	) -> SECURITY_ATTRIBUTES {
		let mut attrs = unsafe { std::mem::zeroed::<SECURITY_ATTRIBUTES>() };
		if let Some(slf) = slf {
			slf.write_to_security_attributes(&mut attrs);
		}
		attrs.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
		attrs.bInheritHandle = inheritable as i32;
		attrs
	}
}

unsafe impl Send for SecurityDescriptor {}
unsafe impl Sync for SecurityDescriptor {}

impl Borrow<SECURITY_DESCRIPTOR> for SecurityDescriptor {
	#[inline]
	fn borrow(&self) -> &SECURITY_DESCRIPTOR {
		&self.0
	}
}
impl AsRef<SECURITY_DESCRIPTOR> for SecurityDescriptor {
	#[inline]
	fn as_ref(&self) -> &SECURITY_DESCRIPTOR {
		&self.0
	}
}

impl Debug for SecurityDescriptor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("SecurityDescriptor")
			.field(&self.0.Revision)
			.field(&self.0.Sbz1)
			.field(&self.0.Control)
			.field(&self.0.Owner)
			.field(&self.0.Group)
			.field(&self.0.Sacl)
			.field(&self.0.Dacl)
			.finish()
	}
}

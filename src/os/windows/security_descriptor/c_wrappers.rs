use std::{ffi::c_void, io, ptr};
use windows_sys::Win32::{
	Foundation::{LocalFree, BOOL, PSID},
	Security::{
		FreeSid, GetSecurityDescriptorControl, SetSecurityDescriptorControl, ACL,
		SECURITY_DESCRIPTOR_CONTROL,
	},
};

pub(super) unsafe fn control_and_revision(
	sd: *const c_void,
) -> io::Result<(SECURITY_DESCRIPTOR_CONTROL, u32)> {
	let mut control = SECURITY_DESCRIPTOR_CONTROL::default();
	let mut revision = 0;
	let success =
		unsafe { GetSecurityDescriptorControl(sd.cast_mut(), &mut control, &mut revision) != 0 };
	ok_or_errno!(success => (control, revision))
}

pub(super) unsafe fn acl(
	sd: *const c_void,
	f: unsafe extern "system" fn(*mut c_void, *mut BOOL, *mut *mut ACL, *mut BOOL) -> BOOL,
) -> io::Result<Option<(*mut ACL, bool)>> {
	let mut exists = 0;
	let mut pacl = ptr::null_mut();
	let mut defaulted = 0;
	let success = unsafe { f(sd.cast_mut(), &mut exists, &mut pacl, &mut defaulted) != 0 };
	ok_or_errno!(success => {
		if exists != 0 {
			Some((pacl, defaulted != 0))
		} else {
			None
		}
	})
}
pub(super) unsafe fn sid(
	sd: *const c_void,
	f: unsafe extern "system" fn(*mut c_void, *mut PSID, *mut BOOL) -> BOOL,
) -> io::Result<(PSID, bool)> {
	let mut psid = ptr::null_mut();
	let mut defaulted = 1;
	let success = unsafe { f(sd.cast_mut(), &mut psid, &mut defaulted) != 0 };
	ok_or_errno!(success => {
		(psid, defaulted != 0)
	})
}

pub(super) unsafe fn set_acl(
	sd: *const c_void,
	acl: Option<*mut ACL>,
	defaulted: bool,
	f: unsafe extern "system" fn(*mut c_void, BOOL, *const ACL, BOOL) -> BOOL,
) -> io::Result<()> {
	let has_acl = acl.is_some() as i32;
	// Note that the null ACL is a valid value that does not represent the lack of an ACL. The null
	// pointer this defaults to will be ignored by Windows because has_acl == false.
	let acl = acl.unwrap_or(ptr::null_mut());
	let success = unsafe { f(sd.cast_mut(), has_acl, acl, defaulted as i32) != 0 };
	ok_or_errno!(success => ())
}
pub(super) unsafe fn set_sid(
	sd: *const c_void,
	sid: PSID,
	defaulted: bool,
	f: unsafe extern "system" fn(*mut c_void, PSID, BOOL) -> BOOL,
) -> io::Result<()> {
	let success = unsafe { f(sd.cast_mut(), sid, defaulted as i32) != 0 };
	ok_or_errno!(success => ())
}

pub(super) unsafe fn set_control(
	sd: *const c_void,
	mask: SECURITY_DESCRIPTOR_CONTROL,
	value: SECURITY_DESCRIPTOR_CONTROL,
) -> io::Result<()> {
	let success = unsafe { SetSecurityDescriptorControl(sd.cast_mut(), mask, value) != 0 };
	ok_or_errno!(success => ())
}

pub(super) unsafe fn unset_acl(
	sd: *const c_void,
	f: unsafe extern "system" fn(*mut c_void, BOOL, *const ACL, BOOL) -> BOOL,
) -> io::Result<()> {
	unsafe { set_acl(sd, None, false, f) }
}
pub(super) unsafe fn unset_sid(
	sd: *const c_void,
	f: unsafe extern "system" fn(*mut c_void, PSID, BOOL) -> BOOL,
) -> io::Result<()> {
	unsafe { set_sid(sd, ptr::null_mut(), false, f) }
}

pub(super) unsafe fn free_acl(acl: *mut ACL) -> io::Result<()> {
	let success = unsafe { LocalFree(acl.cast()).is_null() };
	ok_or_errno!(success => ())
}
pub(super) unsafe fn free_sid(sid: PSID) -> io::Result<()> {
	if sid.is_null() {
		return Ok(());
	}
	let success = unsafe { FreeSid(sid).is_null() };
	if success {
		Ok(())
	} else {
		Err(io::Error::new(
			io::ErrorKind::Other,
			"failed to deallocate SID",
		))
	}
}

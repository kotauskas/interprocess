use crate::OrErrno;

use super::*;
use std::{
	marker::PhantomData,
	mem::{size_of, size_of_val, zeroed, ManuallyDrop},
	ptr::{self, NonNull},
};
use windows_sys::Win32::{
	Foundation::{LocalFree, PSID},
	Security::{
		AclRevisionInformation, AclSizeInformation, AddAce, CopySid, GetAce, GetAclInformation,
		GetSidLengthRequired, GetSidSubAuthorityCount, InitializeAcl, IsValidSid, ACL,
		ACL_INFORMATION_CLASS, ACL_REVISION_INFORMATION, ACL_SIZE_INFORMATION,
		SECURITY_DESCRIPTOR_CONTROL, SE_DACL_DEFAULTED, SE_DACL_PRESENT, SE_GROUP_DEFAULTED,
		SE_OWNER_DEFAULTED, SE_RM_CONTROL_VALID, SE_SELF_RELATIVE,
	},
	System::Memory::{LocalAlloc, LMEM_FIXED},
};

pub(super) unsafe fn clone(sd: *const c_void) -> io::Result<SecurityDescriptor> {
	// Cannot (and should not) set those with SetSecurityDescriptorControl().
	const CONTROL_MASK: SECURITY_DESCRIPTOR_CONTROL = !(SE_SELF_RELATIVE
		| SE_DACL_PRESENT
		| SE_DACL_DEFAULTED
		| SE_OWNER_DEFAULTED
		| SE_GROUP_DEFAULTED
		| SE_RM_CONTROL_VALID);

	let mut new_sd = SecurityDescriptor::new()?;
	let old_sd = unsafe {
		// SAFETY: as per contract
		BorrowedSecurityDescriptor::from_ptr(sd)
	};

	let acl_fn = |(acl, dfl)| io::Result::<(LocalBox, bool)>::Ok((unsafe { clone_acl(acl)? }, dfl));
	let sid_fn =
		|(sid, dfl)| io::Result::<(Option<LocalBox>, bool)>::Ok((unsafe { clone_sid(sid)? }, dfl));
	let dacl = old_sd.dacl()?.map(acl_fn).transpose()?;
	let sacl = old_sd.sacl()?.map(acl_fn).transpose()?;
	let owner = sid_fn(old_sd.owner()?)?;
	let group = sid_fn(old_sd.group()?)?;

	if let Some((acl, dfl)) = dacl {
		let mut acl = ManuallyDrop::new(acl);
		unsafe { new_sd.set_dacl(acl.as_acl(), dfl)? };
	}
	if let Some((acl, dfl)) = sacl {
		let mut acl = ManuallyDrop::new(acl);
		unsafe { new_sd.set_dacl(acl.as_acl(), dfl)? };
	}

	let assid = |sid: &mut LocalBox| sid.as_sid();
	let (mut owner, odfl) = (ManuallyDrop::new(owner.0), owner.1);
	unsafe { new_sd.set_owner(owner.as_mut().map(assid).unwrap_or(ptr::null_mut()), odfl)? };

	let (mut group, gdfl) = (ManuallyDrop::new(group.0), group.1);
	unsafe { new_sd.set_owner(group.as_mut().map(assid).unwrap_or(ptr::null_mut()), gdfl)? };

	let control = old_sd.control_and_revision()?.0;
	new_sd.set_control(CONTROL_MASK, control)?;

	Ok(new_sd)
}

struct LocalBox(NonNull<c_void>, PhantomData<c_void>);
impl LocalBox {
	#[allow(clippy::unwrap_used, clippy::unwrap_in_result)]
	fn allocate(sz: u32) -> io::Result<Self> {
		// Unwrap note: this code isn't supposed to compile on Win16.
		let allocation = unsafe { LocalAlloc(LMEM_FIXED, sz.try_into().unwrap()) };
		(allocation.is_null()).false_or_errno(|| unsafe {
			Self(NonNull::new_unchecked(allocation.cast()), PhantomData)
		})
	}
	fn as_acl(&mut self) -> *mut ACL {
		self.0.as_ptr().cast()
	}
	fn as_sid(&mut self) -> PSID {
		self.0.as_ptr().cast()
	}
}
impl Drop for LocalBox {
	fn drop(&mut self) {
		unsafe { LocalFree(self.0.as_ptr()) }
			.is_null()
			.true_val_or_errno(())
			.expect("LocalFree() failed")
	}
}

/// Wraps `GetAclInformation()`.
///
/// # Safety
/// -	`zeroed::<T>()` must be POD, i.e. all bit patterns of `T`'s size must constitute
/// 	well-initialized instances of `T`.
/// -	`T` must be the correct size for `information_class`.
unsafe fn get_acl_info<T>(
	acl: *mut ACL,
	information_class: ACL_INFORMATION_CLASS,
) -> io::Result<T> {
	let mut info = unsafe { zeroed::<T>() };
	unsafe {
		GetAclInformation(
			acl,
			(&mut info as *mut T).cast(),
			size_of_val(&info) as u32,
			information_class,
		)
		.true_val_or_errno(info)
	}
}

#[allow(clippy::unwrap_used)]
fn create_acl(sz: u32, rev: u32) -> io::Result<LocalBox> {
	const ALIGN: u32 = size_of::<u32>() as u32; // 100₂
	const ALIGN_MASK: u32 = ALIGN - 1; // 011₂
	let sz = if sz & ALIGN_MASK != 0 {
		// It's not possible for the allocated size of an ACL to exceed DWORD::MAX, and it's also
		// not possible for the upward-aligned bytes-in-use figure to exceed the allocated size.
		sz.checked_add(1).unwrap()
	} else {
		sz
	};

	let mut acl = LocalBox::allocate(sz)?;
	unsafe { InitializeAcl(acl.as_acl(), sz, rev) }.true_val_or_errno(acl)
}

unsafe fn clone_acl(acl: *mut ACL) -> io::Result<LocalBox> {
	let (sz_info, rev) = unsafe {
		let sz_info = get_acl_info::<ACL_SIZE_INFORMATION>(acl, AclSizeInformation)?;
		let rev =
			get_acl_info::<ACL_REVISION_INFORMATION>(acl, AclRevisionInformation)?.AclRevision;
		(sz_info, rev)
	};
	let mut new_acl = create_acl(sz_info.AclBytesInUse, rev)?;

	unsafe {
		let mut ace = ptr::null_mut();
		for i in 0..sz_info.AceCount {
			GetAce(new_acl.as_acl(), i, &mut ace).true_val_or_errno(())?;
			AddAce(new_acl.as_acl(), rev, i, ace.cast_const(), 1).true_val_or_errno(())?;
		}
	}
	Ok(new_acl)
}

unsafe fn clone_sid(sid: PSID) -> io::Result<Option<LocalBox>> {
	if sid.is_null() {
		// Unlike with ACLs, a null PSID is a sentinel for the lack of a SID. By analogy with
		// `None.clone() == None`, we return the same value.
		return Ok(None);
	}
	unsafe { IsValidSid(sid) }.true_val_or_errno(())?;

	let num_subauths = unsafe { *GetSidSubAuthorityCount(sid) };
	let sz = unsafe { GetSidLengthRequired(num_subauths) };

	let mut new_sid = LocalBox::allocate(sz)?;

	unsafe { CopySid(sz, new_sid.as_sid(), sid) }.true_val_or_errno(Some(new_sid))
}

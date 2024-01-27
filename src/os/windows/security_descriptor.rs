//! Windows-specific handling for security attributes and descriptors.
//!
//! This module includes the SecurityAttributes struct for defining security characteristics of Windows objects,
//! and functions for initializing security descriptors, which are essential in managing access control and security.
use std::io;
use std::mem::{size_of, zeroed};
use windows_sys::Win32::{
    Security::{SECURITY_ATTRIBUTES, PSECURITY_DESCRIPTOR},
};

/// Represents security attributes that can be applied to objects created by various Windows functions.
/// This structure includes attributes related to security, inheritance, and length specifications.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SecurityAttributes {
    /// The size of the `SecurityAttributes` structure, in bytes.
    pub n_length: u32,
    /// A string that defines the security attributes.
    pub attributes: Option<String>,
    /// A flag indicating whether the handle is inheritable.
    pub inherit_handle: i32,
}

/// Implements default values for `SecurityAttributes`.
/// By default, all fields are set to zero or `None`.
impl Default for SecurityAttributes {
    fn default() -> Self {
        Self {
            n_length: 0,
            attributes: None,
            inherit_handle: 0,
        }
    }
}

/// Provides a constructor for `SecurityAttributes` representing access permissions for any user.
impl SecurityAttributes {
    /// Sets the `attributes` to "Everyone", allowing any user access.
    pub fn any_user(&self) -> Self {
        Self {
            n_length: 0,
            attributes: Some("Everyone".to_string()),
            inherit_handle: 0,
        }
    }
}

/// Provides a method to create an empty `SecurityAttributes` with all fields zeroed.
impl SecurityAttributes {
    /// Returns an instance of `SecurityAttributes` with all fields set to zero.
    pub fn empty() -> SecurityAttributes {
        unsafe { zeroed() }
    }
}


/// Initializes a new `SECURITY_ATTRIBUTES` structure with zeroed values and sets the `nLength` field to the size of the `SECURITY_ATTRIBUTES` type.
///
/// # Returns
/// A `SECURITY_ATTRIBUTES` structure with initialized `nLength`.
pub fn init_security_attributes() -> SECURITY_ATTRIBUTES {
    let mut a: SECURITY_ATTRIBUTES = unsafe { zeroed() };
    a.nLength = size_of::<SECURITY_ATTRIBUTES>() as _;
    a
}

/// Allocates and initializes a new security descriptor in memory. It uses the `InitializeSecurityDescriptor` function to set the revision level to `SECURITY_DESCRIPTOR_REVISION`.
///
/// # Returns
/// A pointer to the initialized security descriptor (`PSECURITY_DESCRIPTOR`).
pub fn init_security_description() -> io::Result<PSECURITY_DESCRIPTOR> {
    let layout = std::alloc::Layout::from_size_align(size_of::<[u8; winapi::um::winnt::SECURITY_DESCRIPTOR_MIN_LENGTH]>() as _, 8).unwrap();
    let p_sd: PSECURITY_DESCRIPTOR = unsafe { std::alloc::alloc(layout) as PSECURITY_DESCRIPTOR };

    // Inicializar el descriptor de seguridad
    let result = unsafe {
        windows_sys::Win32::Security::InitializeSecurityDescriptor(
            p_sd,
            winapi::um::winnt::SECURITY_DESCRIPTOR_REVISION,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(p_sd)
}
/// A [named pipe wait timeout][npw].
///
/// [npw]: https://learn.microsoft.com/en-nz/windows/win32/api/namedpipeapi/nf-namedpipeapi-waitnamedpipew
#[repr(transparent)] // #[repr(u32)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct WaitTimeout(u32);
impl WaitTimeout {
	/// Default wait timeout.
	///
	/// If specified on the client, uses the default wait timeout specified by the server. If
	/// the server also specifies this value, Windows defaults to **50 milliseconds**.
	pub const DEFAULT: Self = Self(0x00000000);
	/// Wait indefinitely.
	pub const FOREVER: Self = Self(0xffffffff);

	/// Constructs from a raw value (given in milliseconds).
	///
	/// See [`DEFAULT`](Self::DEFAULT) and [`FOREVER`](Self::FOREVER).
	#[inline(always)]
	pub const fn from_raw(raw: u32) -> Self {
		Self(raw)
	}
	/// Returns the contained raw value (given in milliseconds).
	///
	/// See [`DEFAULT`](Self::DEFAULT) and [`FOREVER`](Self::FOREVER).
	#[inline(always)]
	pub const fn to_raw(self) -> u32 {
		self.0
	}
}
impl From<WaitTimeout> for u32 {
	#[inline(always)]
	fn from(x: WaitTimeout) -> Self {
		x.to_raw()
	}
}

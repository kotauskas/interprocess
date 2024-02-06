//! Tests what happens when a client attempts to connect to a local socket that doesn't exist.

use crate::{local_socket::LocalSocketStream, tests::util::*};
use color_eyre::eyre::{bail, ensure};
use std::io;

pub fn run_and_verify_error(namespaced: bool) -> TestResult {
	use io::ErrorKind::*;
	let err = match client(namespaced) {
		Err(e) => e,
		Ok(()) => bail!("client successfully connected to nonexistent server"),
	};
	ensure!(
		matches!(err.kind(), NotFound | ConnectionRefused),
		"expected error to be 'not found', received '{}'",
		err
	);
	Ok(())
}
fn client(namespaced: bool) -> io::Result<()> {
	let name = namegen_local_socket(make_id!(), namespaced).next().unwrap();
	LocalSocketStream::connect(name?.borrow())?;
	Ok(())
}

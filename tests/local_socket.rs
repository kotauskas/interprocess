// TODO test various error conditions

mod no_server;
mod stream;

use crate::{
	local_socket::{LocalSocketName, NameTypeSupport},
	tests::util::*,
};

fn test_stream(id: &'static str, split: bool, path: bool) -> TestResult {
	use stream::*;
	testinit();
	let hcl = if split {
		handle_client_split as _
	} else {
		handle_client_nosplit as _
	};
	let client: fn(&LocalSocketName<'_>) -> TestResult = if split {
		client_split as _
	} else {
		client_nosplit as _
	};
	let scl = |s, n| server(id, hcl, s, n, path);
	drive_server_and_multiple_clients(scl, client)?;
	Ok(())
}

macro_rules! matrix {
	(@querymethod true $e:expr) => { NameTypeSupport::fs_supported($e) };
	(@querymethod false $e:expr) => { NameTypeSupport::ns_supported($e) };
	(@body $split:ident $path:ident) => {{
		if matrix!(@querymethod $path NameTypeSupport::query()) {
			test_stream(make_id!(), $split, $path)?;
		}
		Ok(())
	}};
	($nm:ident false $path:ident) => {
		#[test]
		fn $nm() -> TestResult { matrix!(@body false $path) }
	};
	($nm:ident true $path:ident) => {
		#[test]
		#[cfg(not(windows))]
		fn $nm() -> TestResult { matrix!(@body true $path) }
	};
	($($nm:ident $split:ident $path:ident)+) => { $(matrix!($nm $split $path);)+ };
}

matrix! {
	stream_file_nosplit			false	true
	stream_file_split			true	true
	stream_namespaced_nosplit	false	false
	stream_namespaced_split		true	false
}

#[test]
fn no_server_file() -> TestResult {
	testinit();
	if NameTypeSupport::query().fs_supported() {
		no_server::run_and_verify_error(false)?;
	}
	Ok(())
}
#[test]
fn no_server_namespaced() -> TestResult {
	testinit();
	if NameTypeSupport::query().fs_supported() {
		no_server::run_and_verify_error(true)?;
	}
	Ok(())
}

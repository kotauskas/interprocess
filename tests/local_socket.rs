// TODO test various error conditions

mod no_server;
mod stream;

use crate::{local_socket::NameTypeSupport, tests::util::*};

fn test_stream(id: &'static str, path: bool) -> TestResult {
	use stream::*;
	testinit();
	let scl = |s, n| server(id, handle_client, s, n, path);
	drive_server_and_multiple_clients(scl, client)?;
	Ok(())
}

fn test_no_server(id: &'static str, path: bool) -> TestResult {
	testinit();
	no_server::run_and_verify_error(id, path)
}

macro_rules! tests {
	(@querymethod true $e:expr) => { NameTypeSupport::fs_supported($e) };
	(@querymethod false $e:expr) => { NameTypeSupport::ns_supported($e) };
	(@body $fn:ident $path:ident) => {{
		if tests!(@querymethod $path NameTypeSupport::query()) {
			$fn(make_id!(), $path)?;
		}
		Ok(())
	}};
	($fn:ident $nm:ident $path:ident) => {
		#[test]
		fn $nm() -> TestResult { tests!(@body $fn $path) }
	};
	($fn:ident $($nm:ident $path:ident)+) => { $(tests!($fn $nm $path);)+ };
}

tests! {test_stream
	stream_file			true
	stream_namespaced	false
}

tests! {test_no_server
	no_server_file			true
	no_server_namespaced	false
}

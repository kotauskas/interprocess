// TODO test various error conditions
// TODO test reunite in some shape or form
#![cfg(feature = "tokio")]

mod no_server;
mod stream;

use crate::{
	local_socket::NameTypeSupport,
	tests::util::{self, testinit, TestResult},
};

async fn test_stream(id: &'static str, split: bool, nmspc: bool) -> TestResult {
	use stream::*;
	testinit();
	util::tokio::drive_server_and_multiple_clients(move |s, n| server(id, s, n, nmspc), client)
		.await
}

macro_rules! matrix {
	(@querymethod true $e:expr) => { NameTypeSupport::ns_supported($e) };
	(@querymethod false $e:expr) => { NameTypeSupport::fs_supported($e) };
	(@body $split:ident $nmspc:ident) => {{
		if matrix!(@querymethod $nmspc NameTypeSupport::query()) {
			test_stream(make_id!(), $split, $nmspc).await?;
		}
		Ok(())
	}};
	($nm:ident $split:ident $nmspc:ident) => {
		#[tokio::test]
		async fn $nm() -> TestResult { matrix!(@body $split $nmspc) }
	};
	($($nm:ident $split:ident $nmspc:ident)+) => { $(matrix!($nm $split $nmspc);)+ };
}

matrix! {
	stream_file_nosplit			false	false
	stream_file_split			true	false
	stream_namespaced_nosplit	false	true
	stream_namespaced_split		true	true
}

#[tokio::test]
async fn no_server_file() -> TestResult {
	testinit();
	if NameTypeSupport::query().fs_supported() {
		no_server::run_and_verify_error(false).await?;
	}
	Ok(())
}
#[tokio::test]
async fn no_server_namespaced() -> TestResult {
	testinit();
	if NameTypeSupport::query().ns_supported() {
		no_server::run_and_verify_error(true).await?;
	}
	Ok(())
}

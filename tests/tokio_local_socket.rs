// TODO test various error conditions
#![cfg(feature = "tokio")]

mod no_server;
mod stream;

use crate::{
	local_socket::{tokio::LocalSocketStream, LocalSocketName, NameTypeSupport},
	tests::util::{self, testinit, TestResult},
};
use std::{future::Future, pin::Pin, sync::Arc};

#[allow(clippy::type_complexity)]
async fn test_stream(id: &'static str, split: bool, nmspc: bool) -> TestResult {
	use stream::*;
	testinit();
	type Fut = Pin<Box<dyn Future<Output = TestResult> + Send + 'static>>;
	type F<T> = Box<dyn Fn(T) -> Fut + Send + Sync>;
	let hcl: F<LocalSocketStream> = if split {
		Box::new(|conn| Box::pin(handle_client_split(conn)))
	} else {
		Box::new(|conn| Box::pin(handle_client_nosplit(conn)))
	};
	let client: F<Arc<LocalSocketName<'static>>> = if split {
		Box::new(|conn| Box::pin(client_split(conn)))
	} else {
		Box::new(|conn| Box::pin(client_nosplit(conn)))
	};
	util::tokio::drive_server_and_multiple_clients(move |s, n| server(id, hcl, s, n, nmspc), client)
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
	($nm:ident false $nmspc:ident) => {
		#[tokio::test]
		async fn $nm() -> TestResult { matrix!(@body false $nmspc) }
	};
	($nm:ident true $nmspc:ident) => {
		#[tokio::test]
		#[cfg(not(windows))]
		async fn $nm() -> TestResult { matrix!(@body true $nmspc) }
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

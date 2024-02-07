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
async fn test_stream(id: &'static str, split: bool, path: bool) -> TestResult {
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
	util::tokio::drive_server_and_multiple_clients(move |s, n| server(id, hcl, s, n, path), client)
		.await
}

macro_rules! matrix {
	(@querymethod true $e:expr) => { NameTypeSupport::fs_supported($e) };
	(@querymethod false $e:expr) => { NameTypeSupport::ns_supported($e) };
	(@body $split:ident $path:ident) => {{
		if matrix!(@querymethod $path NameTypeSupport::query()) {
			test_stream(make_id!(), $split, $path).await?;
		}
		Ok(())
	}};
	($nm:ident false $path:ident) => {
		#[tokio::test]
		async fn $nm() -> TestResult { matrix!(@body false $path) }
	};
	($nm:ident true $path:ident) => {
		#[tokio::test]
		#[cfg(not(windows))]
		async fn $nm() -> TestResult { matrix!(@body true $path) }
	};
	($($nm:ident $split:ident $path:ident)+) => { $(matrix!($nm $split $path);)+ };
}

matrix! {
	stream_file_nosplit			false	true
	stream_file_split			true	true
	stream_namespaced_nosplit	false	false
	stream_namespaced_split		true	false
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

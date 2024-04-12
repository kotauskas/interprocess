use crate::{
	local_socket::{prelude::*, ListenerOptions, Stream},
	os::unix::local_socket::SpecialDirUdSocket,
	tests::util::*,
};
use std::sync::Arc;

fn test_inner() -> TestResult {
	let mut namegen = NameGen::new(make_id!(), |rnum| {
		format!("interprocess test/fake ns/test-{:08x}.sock", rnum)
			.to_ns_name::<SpecialDirUdSocket>()
			.map(Arc::new)
	});
	let (name, _listener) = listen_and_pick_name(&mut namegen, |nm| {
		ListenerOptions::new().name(nm.borrow()).create_sync()
	})?;
	let name = Arc::try_unwrap(name).unwrap();
	let _ = Stream::connect(name.borrow()).opname("client connect")?;

	Ok(())
}

#[test]
fn local_socket_fake_ns() -> TestResult {
	test_wrapper(test_inner)
}

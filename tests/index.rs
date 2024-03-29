#[path = "util/mod.rs"]
#[macro_use]
mod util;

mod os {
	#[cfg(unix)]
	mod unix {
		mod local_socket_mode;
	}
}

mod local_socket;
mod named_pipe;
mod tokio_local_socket;
mod tokio_named_pipe;

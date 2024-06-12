mod side_a;
mod side_b;

use std::{io, sync::mpsc, thread};

fn main() -> io::Result<()> {
	let (htx, hrx) = mpsc::sync_channel(1);
	let jh = thread::spawn(move || side_a::main(htx));
	let handle = hrx.recv().unwrap();

	side_b::main(handle)?;
	jh.join().unwrap()
}

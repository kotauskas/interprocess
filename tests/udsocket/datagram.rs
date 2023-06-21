use super::util::*;
use color_eyre::eyre::Context;
use interprocess::os::unix::udsocket::UdSocket;
use std::{io, sync::mpsc::Sender};

pub(super) fn run_with_namegen(mut namegen: NameGen) {
    let (a_name, a_socket) = make_socket(&mut namegen).expect("failed to make side A socket");
    let (b_name, b_socket) = make_socket(&mut namegen).expect("failed to make side B socket");

    let side_a = move |s| side(a_socket, Some(s), b_name);
    let side_b = move |_| side(b_socket, None, a_name);
    drive_pair(side_a, "side A", side_b, "side B");
}

fn make_message(side_name: char, second: bool) -> Vec<u8> {
    let fs = if second { "Second" } else { "First" };
    format!("{fs} message from side {side_name}").into_bytes()
}

fn make_socket(namegen: &mut NameGen) -> io::Result<(String, UdSocket)> {
    namegen
        .find_map(|nm| {
            let s = match UdSocket::bind(&*nm) {
                Ok(s) => s,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, s)))
        })
        .unwrap()
}

fn side(sock: UdSocket, notifier: Option<Sender<()>>, other_name: String) -> TestResult {
    let (mut buf1, mut buf2) = ([0; 64], [0; 64]);

    let (side_name, other_side_name) = if let Some(n) = notifier {
        let _ = n.send(());
        ('A', 'B')
    } else {
        ('B', 'A')
    };
    let own_msg_1 = make_message(side_name, false);
    let own_msg_2 = make_message(side_name, true);
    let other_msg_1 = make_message(other_side_name, false);
    let other_msg_2 = make_message(other_side_name, true);

    sock.set_destination(other_name).context("Set destination failed")?;

    let written = sock.send(&own_msg_1).context("First socket send failed")?;
    assert_eq!(written, own_msg_1.len());

    let written = sock.send(&own_msg_2).context("Second socket send failed")?;
    assert_eq!(written, own_msg_2.len());

    let read = sock.recv(&mut buf1).context("First socket receive failed")?;
    assert_eq!(read, other_msg_1.len());
    assert_eq!(&buf1[0..read], other_msg_1);

    let read = sock.recv(&mut buf2).context("Second socket receive failed")?;
    assert_eq!(read, other_msg_2.len());
    assert_eq!(&buf2[0..read], other_msg_2);

    Ok(())
}

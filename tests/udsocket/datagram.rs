use super::util::*;
use color_eyre::eyre::Context;
use interprocess::os::unix::udsocket::UdDatagram;
use std::sync::mpsc::Sender;

pub(super) fn run(mut namegen: NameGen) -> TestResult {
    let mks = |nm: &str| UdDatagram::bound(nm);
    let (a_name, a_socket) = listen_and_pick_name(&mut namegen, mks).context("failed to make side A socket")?;
    let (b_name, b_socket) = listen_and_pick_name(&mut namegen, mks).context("failed to make side B socket")?;

    let side_a = move |s| side(a_socket, Some(s), b_name);
    let side_b = move |_| side(b_socket, None, a_name);
    drive_pair(side_a, "side A", side_b, "side B")
}

fn make_message(side_name: char, second: bool) -> Vec<u8> {
    let fs = if second { "Second" } else { "First" };
    format!("{fs} message from side {side_name}").into_bytes()
}

fn side(sock: UdDatagram, notifier: Option<Sender<()>>, other_name: String) -> TestResult {
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

    sock.set_destination(other_name).context("set destination failed")?;

    let written = sock.send(&own_msg_1).context("first socket send failed")?;
    ensure_eq!(written, own_msg_1.len());

    let written = sock.send(&own_msg_2).context("second socket send failed")?;
    ensure_eq!(written, own_msg_2.len());

    let read = sock.recv(&mut buf1).context("first socket receive failed")?;
    ensure_eq!(read, other_msg_1.len());
    ensure_eq!(&buf1[0..read], other_msg_1);

    let read = sock.recv(&mut buf2).context("second socket receive failed")?;
    ensure_eq!(read, other_msg_2.len());
    ensure_eq!(&buf2[0..read], other_msg_2);

    Ok(())
}

#![cfg(uds_credentials)]

use super::util::*;
use color_eyre::eyre::{bail, Context};
use interprocess::os::unix::udsocket::{
    cmsg::{ancillary::credentials::Credentials, Cmsg, CmsgMutExt, CmsgRef, CmsgVecBuf},
    ReadAncillaryExt, UdSocket, UdStream, UdStreamListener, WriteAncillaryExt,
};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::Shutdown,
    sync::{mpsc::Sender, Arc},
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub(super) fn run(namegen: NameGen, contcred: bool) {
    drive_server_and_multiple_clients(
        move |snd, nc| server(snd, nc, namegen, false, contcred),
        move |nm| client(nm, false, contcred),
    );
    drive_server_and_multiple_clients(
        move |snd, nc| server(snd, nc, namegen, true, contcred),
        move |nm| client(nm, true, contcred),
    );
}

fn enable_passcred(sock: &UdStream) -> TestResult {
    #[cfg(uds_cont_credentials)]
    {
        sock.set_continuous_ancillary_credentials(true)
            .context("Failed to enable credential passing")
    }

    #[cfg(not(uds_cont_credentials))]
    {
        bail!("Attempted to enable credential passing on a platform that doesn't support it (misconfigured test)")
    }
}
fn decreds(abuf: CmsgRef<'_>) -> TestResult<Credentials<'_>> {
    match abuf.decode::<Credentials>().next() {
        Some(Ok(c)) => Ok(c),
        Some(Err(e)) => bail!("Parsing of credentials failed: {e}"),
        None => bail!("No credentials received"),
    }
}
fn ckcreds(creds: &Credentials) {
    if let Some(pid) = creds.pid() {
        assert_eq!(pid, unsafe { libc::getpid() });
    }
    assert_eq!(creds.best_effort_ruid(), unsafe { libc::getuid() });
    assert_eq!(creds.best_effort_rgid(), unsafe { libc::getgid() });
}

fn server(
    name_sender: Sender<String>,
    num_clients: u32,
    mut namegen: NameGen,
    shutdown: bool,
    contcred: bool,
) -> TestResult {
    let (name, listener) = namegen
        .find_map(|nm| {
            let l = match UdStreamListener::bind(&*nm) {
                Ok(l) => l,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap()
        .context("Listener bind failed")?;

    let _ = name_sender.send(name);

    let mut buffer = String::with_capacity(128);

    let mut abm = CmsgVecBuf::new(0);
    if !contcred {
        let _ = &mut abm;
        #[cfg(uds_cmsgcred)]
        {
            abm.add_message(&Credentials::sendable_cmsgcred());
        }
    }
    let ancself = abm.as_ref();

    let mut abread = CmsgVecBuf::new(Cmsg::cmsg_len_for_payload_size(Credentials::MIN_ANCILLARY_SIZE) * 8);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c.with_cmsg_mut_by_val(&mut abread)),
            Err(e) => bail!("Incoming connection failed: {e}"),
        };
        if contcred {
            enable_passcred(&conn.get_mut().reader).context("Passcred enable failed")?;
        }

        if shutdown {
            conn.read_to_string(&mut buffer)
        } else {
            conn.read_line(&mut buffer)
        }
        .context("Socket receive failed")?;

        let mut conn = conn.into_inner().into_inner().with_cmsg_ref_by_val(ancself);

        conn.write_all(SERVER_MSG.as_bytes()).context("Socket send failed")?;
        if shutdown {
            conn.shutdown(Shutdown::Write)
                .context("Shutdown of writing end failed")?;
        }

        assert_eq!(buffer, CLIENT_MSG);

        let client_creds = decreds(abread.as_ref())?;
        ckcreds(&client_creds);

        buffer.clear();
        abread.clear();
    }
    Ok(())
}

fn client(name: Arc<String>, shutdown: bool, contcred: bool) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let mut abm = CmsgVecBuf::new(0);
    if !contcred {
        let _ = &mut abm;
        #[cfg(uds_cmsgcred)]
        {
            abm.add_message(&Credentials::sendable_cmsgcred());
        }
    }
    let ancself = abm.as_ref();

    let mut abread = CmsgVecBuf::new(Cmsg::cmsg_len_for_payload_size(Credentials::MIN_ANCILLARY_SIZE) * 8);

    let mut conn = UdStream::connect(name.as_str())
        .context("Connect failed")?
        .with_cmsg_ref_by_val(ancself);
    if contcred {
        enable_passcred(&conn.writer).context("Passcred enable failed")?;
    }

    conn.write_all(CLIENT_MSG.as_bytes()).context("Socket send failed")?;

    if shutdown {
        conn.shutdown(Shutdown::Write)
            .context("Shutdown of writing end failed")?;
    }

    let mut conn = BufReader::new(conn.into_inner().with_cmsg_mut_by_val(&mut abread));

    if shutdown {
        conn.read_to_string(&mut buffer)
    } else {
        conn.read_line(&mut buffer)
    }
    .context("Socket receive failed")?;

    assert_eq!(buffer, SERVER_MSG);

    let server_creds = decreds(abread.as_ref())?;
    ckcreds(&server_creds);

    Ok(())
}

#![cfg(uds_credentials)]

use super::util::*;
use color_eyre::eyre::{bail, Context};
use interprocess::os::unix::udsocket::{
    cmsg::{ancillary::credentials::Credentials, Cmsg, CmsgMutExt, CmsgRef, CmsgVecBuf},
    ReadAncillaryExt, UdSocket, UdStream, UdStreamListener, WriteAncillaryExt,
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::Shutdown,
    sync::mpsc::Sender,
};

static SERVER_MSG: &str = "Hello from server!\n";
static CLIENT_MSG: &str = "Hello from client!\n";

pub(super) fn run(namegen: NameGen, contcred: bool) -> TestResult {
    drive_server_and_multiple_clients(
        move |snd, nc| server(snd, nc, namegen, false, contcred),
        move |nm| client(nm, false, contcred),
    )?;
    drive_server_and_multiple_clients(
        move |snd, nc| server(snd, nc, namegen, true, contcred),
        move |nm| client(nm, true, contcred),
    )
}

fn enable_passcred(sock: &UdStream) -> TestResult {
    #[cfg(uds_cont_credentials)]
    {
        sock.set_continuous_ancillary_credentials(true)
            .context("failed to enable credential passing")
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
fn ckcreds(creds: &Credentials) -> TestResult {
    if let Some(pid) = creds.pid() {
        ensure_eq!(pid, unsafe { libc::getpid() });
    }
    ensure_eq!(creds.best_effort_ruid(), unsafe { libc::getuid() });
    ensure_eq!(creds.best_effort_rgid(), unsafe { libc::getgid() });
    Ok(())
}

fn server(
    name_sender: Sender<String>,
    num_clients: u32,
    mut namegen: NameGen,
    shutdown: bool,
    contcred: bool,
) -> TestResult {
    let (name, listener) = listen_and_pick_name(&mut namegen, |nm| UdStreamListener::bind(nm))?;

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
            Err(e) => bail!("incoming connection failed: {e}"),
        };
        if contcred {
            enable_passcred(&conn.get_mut().reader).context("passcred enable failed")?;
        }

        if shutdown {
            conn.read_to_string(&mut buffer)
        } else {
            conn.read_line(&mut buffer)
        }
        .context("socket receive failed")?;

        let mut conn = conn.into_inner().into_inner().with_cmsg_ref_by_val(ancself);

        conn.write_all(SERVER_MSG.as_bytes()).context("socket send failed")?;
        if shutdown {
            conn.shutdown(Shutdown::Write)
                .context("shutdown of writing end failed")?;
        }

        ensure_eq!(buffer, CLIENT_MSG);

        let client_creds = decreds(abread.as_ref())?;
        ckcreds(&client_creds)?;

        buffer.clear();
        abread.clear();
    }
    Ok(())
}

fn client(name: &str, shutdown: bool, contcred: bool) -> TestResult {
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

    let mut conn = UdStream::connect(name)
        .context("connect failed")?
        .with_cmsg_ref_by_val(ancself);
    if contcred {
        enable_passcred(&conn.writer).context("passcred enable failed")?;
    }

    conn.write_all(CLIENT_MSG.as_bytes()).context("socket send failed")?;

    if shutdown {
        conn.shutdown(Shutdown::Write)
            .context("shutdown of writing end failed")?;
    }

    let mut conn = BufReader::new(conn.into_inner().with_cmsg_mut_by_val(&mut abread));

    if shutdown {
        conn.read_to_string(&mut buffer)
    } else {
        conn.read_line(&mut buffer)
    }
    .context("socket receive failed")?;

    ensure_eq!(buffer, SERVER_MSG);

    let server_creds = decreds(abread.as_ref())?;
    ckcreds(&server_creds)?;

    Ok(())
}

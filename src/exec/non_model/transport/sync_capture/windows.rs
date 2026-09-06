// Private local capture endpoints: overlapped readers, synchronous child writers.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};

use super::names::{Stream, next_name};

// Win32 SECURITY_IDENTIFICATION; OpenOptionsExt adds SECURITY_SQOS_PRESENT.
// Do not permit a pipe endpoint to impersonate the service beyond identification.
const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;

pub(super) async fn pair(stream: Stream) -> io::Result<(NamedPipeServer, File)> {
    let name = next_name(stream)?;
    let server = ServerOptions::new()
        .access_inbound(true)
        .access_outbound(false)
        .pipe_mode(PipeMode::Byte)
        .first_pipe_instance(true)
        .max_instances(1)
        .reject_remote_clients(true)
        .create(&name)?;
    // Default server security attributes make its reader non-inheritable,
    // not a private ACL. The sole first instance stays owned here while our
    // synchronous writer opens it. A collision or an already-connected client
    // makes this open fail: never accept that client or retry an existing name.
    let writer = OpenOptions::new()
        .write(true)
        .share_mode(0)
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(&name)?;
    // Opening our own writer is required before accepting the connection.
    // Unlike Tokio ClientOptions this handle is not FILE_FLAG_OVERLAPPED;
    // ordinary child stdout/stderr writes therefore use synchronous handles.
    server.connect().await?;
    Ok((server, writer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn owned_synchronous_writer_delivers_bytes_and_real_eof() {
        let (mut reader, mut writer) = pair(Stream::Stdout).await.expect("private pipe pair");
        writer
            .write_all(b"finite diagnostic\n")
            .expect("finite write");
        drop(writer);
        let mut bytes = Vec::new();
        tokio::time::timeout(Duration::from_secs(1), reader.read_to_end(&mut bytes))
            .await
            .expect("EOF follows the last owned writer")
            .expect("read owned pipe");
        assert_eq!(bytes, b"finite diagnostic\n");
    }

    #[tokio::test]
    async fn owned_pending_reader_can_be_cancelled_and_dropped() {
        let (mut reader, writer) = pair(Stream::Stderr).await.expect("private pipe pair");
        let mut bytes = Vec::new();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), reader.read_to_end(&mut bytes))
                .await
                .is_err()
        );
        drop(reader);
        drop(writer);
        assert!(bytes.is_empty());
    }
}

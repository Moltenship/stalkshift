//! Async operations run only outside SCS callbacks. Any partial operation timeout
//! terminates the whole connection; an incomplete frame is never reused.
use crate::{FRAME_SIZE, IO_TIMEOUT, Packet};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

pub fn runtime() -> io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

pub fn server(name: &str) -> io::Result<NamedPipeServer> {
    ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .max_instances(1)
        .create(name)
}

pub async fn send(pipe: &mut (impl AsyncWrite + Unpin), packet: Packet) -> io::Result<()> {
    tokio::time::timeout(IO_TIMEOUT, pipe.write_all(&packet.encode()))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "pipe write deadline"))?
}

pub async fn receive(pipe: &mut (impl AsyncRead + Unpin)) -> io::Result<Packet> {
    let mut bytes = [0; FRAME_SIZE];
    tokio::time::timeout(IO_TIMEOUT, pipe.read_exact(&mut bytes))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "pipe read deadline"))??;
    Packet::decode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kind;
    use tokio::net::windows::named_pipe::ClientOptions;

    #[test]
    fn real_named_pipe_round_trip_and_single_instance() {
        runtime().unwrap().block_on(async {
            let name = format!(r"\\.\pipe\stalkshift-test-{}", std::process::id());
            let mut server = server(&name).unwrap();
            assert!(super::server(&name).is_err());
            let mut client = ClientOptions::new().open(&name).unwrap();
            server.connect().await.unwrap();
            let packet = Packet {
                kind: Kind::Status,
                session: 1,
                epoch: 1,
                sequence: 0,
                value: 1,
            };
            send(&mut client, packet).await.unwrap();
            assert_eq!(receive(&mut server).await.unwrap(), packet);
            send(&mut server, packet.reply(1)).await.unwrap();
            assert_eq!(receive(&mut client).await.unwrap().value, 1);
            drop(client);
            assert!(receive(&mut server).await.is_err());
        });
    }
    #[test]
    fn partial_frame_times_out_instead_of_being_accepted() {
        runtime().unwrap().block_on(async {
            let (mut writer, mut reader) = tokio::io::duplex(FRAME_SIZE);
            writer.write_all(b"STSF").await.unwrap();
            assert_eq!(
                receive(&mut reader).await.unwrap_err().kind(),
                io::ErrorKind::TimedOut
            );
        });
    }
}

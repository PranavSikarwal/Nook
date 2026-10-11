use crate::config::Config;
use std::io;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(windows)]
use tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(unix)]
pub type LocalIpcStream = UnixStream;
#[cfg(windows)]
pub type LocalIpcStream = NamedPipeClient;

pub async fn connect() -> io::Result<LocalIpcStream> {
    connect_to(&Config::default_socket_path()).await
}

#[cfg(unix)]
async fn connect_to(path: &Path) -> io::Result<LocalIpcStream> {
    tokio::net::UnixStream::connect(path).await
}

#[cfg(windows)]
async fn connect_to(path: &Path) -> io::Result<LocalIpcStream> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(path)
}

pub fn split<S>(stream: S) -> (tokio::io::ReadHalf<S>, tokio::io::WriteHalf<S>)
where
    S: AsyncRead + AsyncWrite,
{
    tokio::io::split(stream)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn client_stream_splits_for_bidirectional_io() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("nook.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(b"pong").await.unwrap();
        });

        let stream = connect_to(&socket_path).await.unwrap();
        let (mut reader, mut writer) = split(stream);
        writer.write_all(b"ping").await.unwrap();
        let mut response = [0; 4];
        reader.read_exact(&mut response).await.unwrap();
        assert_eq!(&response, b"pong");

        server.await.unwrap();
    }
}

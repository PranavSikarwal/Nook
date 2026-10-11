use crate::config::Config;
use std::io;
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
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(Config::default_socket_path()).await
    }

    #[cfg(windows)]
    {
        tokio::net::windows::named_pipe::ClientOptions::new().open(Config::default_socket_path())
    }
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn client_stream_splits_for_bidirectional_io() {
        let listener = UnixListener::bind(Config::default_socket_path()).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(b"pong").await.unwrap();
        });

        let stream = connect().await.unwrap();
        let (mut reader, mut writer) = split(stream);
        writer.write_all(b"ping").await.unwrap();
        let mut response = [0; 4];
        reader.read_exact(&mut response).await.unwrap();
        assert_eq!(&response, b"pong");

        server.await.unwrap();
        let _ = std::fs::remove_file(Config::default_socket_path());
    }
}

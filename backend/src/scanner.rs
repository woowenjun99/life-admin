use std::{io, net::SocketAddr, time::Duration};

use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

const CLAMD_TIMEOUT: Duration = Duration::from_secs(10);
const CLAMD_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanResult {
    Clean,
    Unsafe,
}

#[async_trait]
pub trait FileScanner: Send + Sync {
    async fn scan(&self, content: &[u8]) -> anyhow::Result<ScanResult>;
}

#[derive(Clone)]
pub struct ClamdScanner {
    address: SocketAddr,
}

impl ClamdScanner {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }
}

#[async_trait]
impl FileScanner for ClamdScanner {
    async fn scan(&self, content: &[u8]) -> anyhow::Result<ScanResult> {
        let mut stream = timeout(CLAMD_TIMEOUT, TcpStream::connect(self.address))
            .await
            .map_err(|_| anyhow::anyhow!("ClamAV connection timed out"))??;

        timeout(CLAMD_TIMEOUT, write_instream(&mut stream, content))
            .await
            .map_err(|_| anyhow::anyhow!("ClamAV upload timed out"))??;

        let reply = timeout(CLAMD_TIMEOUT, read_reply(&mut stream))
            .await
            .map_err(|_| anyhow::anyhow!("ClamAV reply timed out"))??;

        classify_reply(&reply).ok_or_else(|| anyhow::anyhow!("unexpected ClamAV reply"))
    }
}

async fn write_instream(stream: &mut TcpStream, content: &[u8]) -> io::Result<()> {
    stream.write_all(b"zINSTREAM\0").await?;

    for chunk in content.chunks(CLAMD_CHUNK_SIZE) {
        stream
            .write_all(&(chunk.len() as u32).to_be_bytes())
            .await?;
        stream.write_all(chunk).await?;
    }

    stream.write_all(&0_u32.to_be_bytes()).await?;
    stream.flush().await
}

async fn read_reply(stream: &mut TcpStream) -> io::Result<String> {
    let mut reply = Vec::with_capacity(128);
    loop {
        let byte = stream.read_u8().await?;
        if byte == 0 {
            break;
        }

        if reply.len() >= 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ClamAV reply exceeded 4 KiB",
            ));
        }
        reply.push(byte);
    }

    String::from_utf8(reply).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn classify_reply(reply: &str) -> Option<ScanResult> {
    if reply.ends_with(" OK") {
        Some(ScanResult::Clean)
    } else if reply.ends_with(" FOUND") {
        Some(ScanResult::Unsafe)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::{ClamdScanner, FileScanner, ScanResult, classify_reply};

    #[test]
    fn classifies_clamd_replies_without_trusting_unknown_replies() {
        assert_eq!(classify_reply("stream: OK"), Some(ScanResult::Clean));
        assert_eq!(
            classify_reply("stream: Eicar-Test-Signature FOUND"),
            Some(ScanResult::Unsafe)
        );
        assert_eq!(classify_reply("stream: scan error"), None);
    }

    #[tokio::test]
    async fn sends_framed_zinstream_content_and_accepts_clean_reply() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener should have an address");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("test client should connect");
            let mut command = [0_u8; 10];
            stream
                .read_exact(&mut command)
                .await
                .expect("scan command should be written");
            assert_eq!(&command, b"zINSTREAM\0");

            let mut received = Vec::new();
            loop {
                let mut length = [0_u8; 4];
                stream
                    .read_exact(&mut length)
                    .await
                    .expect("chunk length should be written");
                let length = u32::from_be_bytes(length) as usize;
                if length == 0 {
                    break;
                }
                let mut chunk = vec![0_u8; length];
                stream
                    .read_exact(&mut chunk)
                    .await
                    .expect("chunk body should be written");
                received.extend(chunk);
            }
            assert_eq!(received, b"private upload content");
            stream
                .write_all(b"stream: OK\0")
                .await
                .expect("clean reply should be written");
        });

        let result = ClamdScanner::new(address)
            .scan(b"private upload content")
            .await
            .expect("scanner response should be classified");

        assert_eq!(result, ScanResult::Clean);
        server.await.expect("test scanner should finish");
    }
}

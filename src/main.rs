mod data_types;

use std::io::Cursor;

use data_types::{VarInt, VarIntError};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info};

#[derive(Debug)]
struct UncompressedPacket<'d> {
    length: VarInt,
    packet_id: VarInt,
    data: &'d [u8],
}

#[derive(Debug, Error)]
enum UncompressedPacketError {
    #[error("Packet length is invalid")]
    InvalidLength(#[source] VarIntError),
    #[error("Packet id is invalid: {0}")]
    InvalidPacketId(#[source] VarIntError),
}

async fn into_uncompressed_dirty(
    packet: &[u8],
) -> Result<UncompressedPacket, UncompressedPacketError> {
    let mut cursor = Cursor::new(packet);
    let length = VarInt::read(&mut cursor)
        .await
        .map_err(UncompressedPacketError::InvalidLength)?;

    let packet_id = VarInt::read(&mut cursor)
        .await
        .map_err(UncompressedPacketError::InvalidPacketId)?;

    Ok(UncompressedPacket {
        length,
        packet_id,
        data: &packet[cursor.position() as usize..],
    })
}

async fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    let mut length_buffer = [0; 5];
    stream.peek(&mut length_buffer[..]).await?;
    let mut length_cursor = Cursor::new(&length_buffer[..]);
    let length = VarInt::read(&mut length_cursor).await?;

    let mut buffer = vec![0; length.0 as usize + length_cursor.position() as usize];
    match stream.read_exact(&mut buffer[..]).await {
        Ok(_) => (),
        Err(e) => {
            error!("{:?}", e);
            info!("Buffer dump: {:?}", buffer);
        }
    };
    let packet = into_uncompressed_dirty(&buffer[..]).await;

    debug!("{:?}", buffer);
    debug!("{:?}", packet);

    stream.write_all(&buffer[..]).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:25565").await?;
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(async move {
            info!("addr: {:?} | Tcp: {:?}", addr, stream);
            match handle_socket(stream).await {
                Ok(_) => (),
                Err(e) => error!("{:?}", e),
            };
        })
        .await?;
    }
}

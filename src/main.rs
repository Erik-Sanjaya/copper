mod data_types;
mod handshaking;

use std::io::Cursor;

use data_types::{VarInt, VarIntError};
use serde_json::json;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info};

use crate::handshaking::Handshaking;

// This is mostly only used for debugging. Although I doubt it'd be done that
// many times using this. But I'll keep it anyway
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

async fn stream_into_vec(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let mut length_buffer = [0; 5];
    stream.peek(&mut length_buffer[..]).await?;
    let mut length_cursor = Cursor::new(&length_buffer[..]);
    let length = VarInt::read(&mut length_cursor).await?;

    let mut buffer = vec![0; length.0 as usize + length_cursor.position() as usize];
    match stream.read_exact(&mut buffer[..]).await {
        Ok(_) => (),
        Err(e) => {
            error!("{:?}", e);
            debug!("Buffer dump: {:?}", buffer);
        }
    };

    Ok(buffer)
}

async fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    let buffer = stream_into_vec(&mut stream).await?;
    let mut cursor = Cursor::new(buffer.as_slice());
    let handshake = Handshaking::read(&mut cursor).await?;
    // let packet = into_uncompressed_dirty(&buffer[..]).await;

    debug!("{:?}", buffer);
    debug!("{:?}", handshake);

    // TODO: implement all of these into actual struct.
    let mut response_buffer_test = vec![];

    let dummy_json_string = json!({
        "version": {
            "name": "1",
            "protocol": 764
        },
        "players": {
            "max": 100,
            "online": 5,
            "sample": [
                {
                    "name": "thinkofdeath",
                    "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
                }
            ]
        },
        "description": {
            "text": "Hello world"
        },
        "favicon": "data:image/png;base64,<data>",
        "enforcesSecureChat": true,
        "previewsChat": true
    })
    .to_string();

    // packet length, 3 is hardcoded from 1 byte for packet id and 2 bytes for
    // string length (VarInt)
    VarInt(3 + dummy_json_string.len() as i32)
        .write(&mut response_buffer_test)
        .await;

    // packet id
    VarInt(0).write(&mut response_buffer_test).await;

    VarInt(dummy_json_string.len() as i32)
        .write(&mut response_buffer_test)
        .await;

    response_buffer_test.extend_from_slice(dummy_json_string.as_bytes());

    stream.write_all(&response_buffer_test).await?;
    debug!("WROTE DUMMY JSON RESPONSE: {:?}", response_buffer_test);
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
        let (stream, _addr) = listener.accept().await?;
        tokio::spawn(async move {
            info!("{:?}", stream);
            let _ = handle_socket(stream)
                .await
                .map_err(|err| error!("{:?}", err));
        })
        .await?;
    }
}

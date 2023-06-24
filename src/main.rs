mod data_types;
mod handshaking;

use std::io::Cursor;

use data_types::{VarInt, VarIntError};
use serde_json::json;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener, TcpStream,
    },
};
use tracing::{debug, error, info};

use crate::handshaking::{Handshaking, HandshakingNextState};

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

async fn stream_into_vec(stream: &mut ReadHalf<'_>) -> anyhow::Result<Vec<u8>> {
    let mut length_buffer = [0; 5];
    stream.peek(&mut length_buffer[..]).await?;
    let mut length_cursor = Cursor::new(&length_buffer[..]);
    let length = VarInt::read(&mut length_cursor).await?;

    let mut buffer = vec![0; length.0 as usize + length_cursor.position() as usize];
    match stream.read_exact(&mut buffer[..]).await {
        Ok(_) => (),
        Err(e) => {
            error!("{:?}", e);
            debug!("Buffer length: {:?}", length);
            debug!("Buffer dump: {:?}", buffer);
        }
    };

    Ok(buffer)
}

enum State {
    Handshaking,
    Status,
    Login,
    Play,
}

async fn handle_handshaking(
    cursor: &mut Cursor<&[u8]>,
    state: &mut State,
) -> anyhow::Result<Handshaking> {
    let handshake = Handshaking::read(cursor).await?;
    debug!("{:?}", handshake);

    match handshake.next_state {
        HandshakingNextState::Status => *state = State::Status,
        HandshakingNextState::Login => *state = State::Login,
    }

    Ok(handshake)
}

async fn handle_status(
    cursor: &mut Cursor<&[u8]>,
    writer: &mut WriteHalf<'_>,
    state: &mut State,
) -> anyhow::Result<()> {
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

    let packet_id = VarInt(0);
    let string_len = VarInt(dummy_json_string.len() as i32);
    let packet_len =
        VarInt((packet_id.size() + string_len.size() + dummy_json_string.len()) as i32);

    packet_len.write(&mut response_buffer_test).await;
    packet_id.write(&mut response_buffer_test).await;

    string_len.write(&mut response_buffer_test).await;
    response_buffer_test.extend_from_slice(dummy_json_string.as_bytes());

    writer.write_all(&response_buffer_test).await?;

    Ok(())
}

async fn handle_login(
    cursor: &mut Cursor<&[u8]>,
    writer: &mut WriteHalf<'_>,
    state: &mut State,
) -> anyhow::Result<()> {
    // TODO: implement all of these into actual struct.
    let mut reply: Vec<u8> = vec![];

    let reply_json = json!({
        "text": "WIP",

    })
    .to_string();

    let packet_id = VarInt(0);
    let string_len = VarInt(reply_json.len() as i32);
    let packet_len = VarInt((packet_id.size() + string_len.size() + reply_json.len()) as i32);

    packet_len.write(&mut reply).await;
    packet_id.write(&mut reply).await;

    string_len.write(&mut reply).await;
    reply.extend_from_slice(reply_json.as_bytes());

    writer.write_all(&reply).await?;
    info!("WRITE LOGIN REPLY");
    writer.shutdown().await?;

    Ok(())
}

async fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    let (mut reader, mut writer) = stream.split();
    let mut state = State::Handshaking;

    loop {
        reader.readable().await?;

        let buffer = stream_into_vec(&mut reader).await?;

        debug!("{:?}", buffer);

        let mut cursor = Cursor::new(buffer.as_slice());

        match state {
            State::Handshaking => {
                let _ = handle_handshaking(&mut cursor, &mut state).await?;
            }
            State::Status => {
                let _ = handle_status(&mut cursor, &mut writer, &mut state).await?;
            }
            State::Login => {
                let _ = handle_login(&mut cursor, &mut writer, &mut state).await?;
            }
            State::Play => {
                info!("ENTERING PLAY STATE");
            }
        };
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:25565").await?;
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .with_file(true)
        .with_line_number(true)
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

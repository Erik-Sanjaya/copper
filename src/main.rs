use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type BytesRead = usize;

#[derive(Debug, Error)]
enum DecodeVarIntError {
    #[error("Bytes exceeded the limit of VarInt")]
    Overflow,
    #[error("Bytes too short")]
    MissingBytes,
}

fn decode_var_int(mut bytes: &[u8]) -> Result<(i32, BytesRead), DecodeVarIntError> {
    let mut result = 0;
    let mut shift = 0;

    loop {
        if shift >= 32 {
            return Err(DecodeVarIntError::Overflow);
        }

        let byte = match bytes.first() {
            Some(b) => *b,
            None => return Err(DecodeVarIntError::MissingBytes),
        };

        bytes = &bytes[1..];
        result |= ((byte & 0x7F) as i32) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }
    }

    Ok((result, shift / 7))
}

#[derive(Debug)]
struct UncompressedPacket<'d> {
    length: i32,
    packet_id: i32,
    data: &'d [u8],
}

#[derive(Debug, Error)]
enum UncompressedPacketError {
    #[error("Packet length is invalid")]
    InvalidLength(#[source] DecodeVarIntError),
    #[error("Packet id is invalid: {0}")]
    InvalidPacketId(#[source] DecodeVarIntError),
}

fn into_uncompressed_dirty(
    mut packet: &[u8],
) -> Result<UncompressedPacket, UncompressedPacketError> {
    let (length, length_bytes) =
        decode_var_int(packet).map_err(UncompressedPacketError::InvalidLength)?;
    packet = &packet[length_bytes..];

    let (packet_id, packet_id_bytes) =
        decode_var_int(packet).map_err(UncompressedPacketError::InvalidPacketId)?;
    packet = &packet[packet_id_bytes..];

    Ok(UncompressedPacket {
        length,
        packet_id,
        data: packet,
    })
}

#[allow(clippy::unused_io_amount)]
async fn handle_socket(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; 128];
    let n = stream.read(&mut buffer[..]).await?;
    let packet = into_uncompressed_dirty(&buffer[..]);

    println!("{:?}", buffer);
    println!("{:?}", packet);
    // println!("{n}");

    stream.write_all(&buffer[..n]).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:25565").await.unwrap();

    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(async move {
            println!("addr: {:?} | Tcp: {:?}", addr, stream);
            handle_socket(stream).await.unwrap();
        })
        .await?;
    }
}

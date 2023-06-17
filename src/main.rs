use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info};

#[derive(Debug, Error)]
enum DecodeVarIntError {
    #[error("Bytes exceeded the limit of VarInt")]
    Overflow,
    #[error("Bytes too short")]
    MissingBytes,
}

type BytesRead = usize;

struct VarInt(i32);

impl VarInt {
    fn read_from(mut bytes: &[u8]) -> Result<(i32, BytesRead), DecodeVarIntError> {
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
        VarInt::read_from(packet).map_err(UncompressedPacketError::InvalidLength)?;
    packet = &packet[length_bytes..];

    let (packet_id, packet_id_bytes) =
        VarInt::read_from(packet).map_err(UncompressedPacketError::InvalidPacketId)?;
    packet = &packet[packet_id_bytes..];

    Ok(UncompressedPacket {
        length,
        packet_id,
        data: packet,
    })
}

async fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    let mut length_buffer = [0; 5];
    stream.peek(&mut length_buffer[..]).await?;
    let length = VarInt::read_from(&length_buffer[..])?;

    let mut buffer = vec![0; length.0 as usize + length.1];
    match stream.read_exact(&mut buffer[..]).await {
        Ok(_) => (),
        Err(e) => {
            error!("{:?}", e);
            info!("Buffer dump: {:?}", buffer);
        }
    };
    let packet = into_uncompressed_dirty(&buffer[..]);

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

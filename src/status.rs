use std::io::Cursor;

use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::WriteHalf,
};

use crate::data_types::{VarInt, VarIntError};
use thiserror::Error;

#[derive(Debug)]
pub struct Status {
    pub packet_id: StatusPacketId,
    pub payload: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StatusPacketId {
    Status = 0x00,
    Ping = 0x01,
}

impl StatusPacketId {
    fn to_varint(&self) -> VarInt {
        match self {
            Self::Status => VarInt(0x00),
            Self::Ping => VarInt(0x01),
        }
    }
}

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("Packet length is invalid: {0}")]
    Length(#[source] VarIntError),
    #[error("Error with packet id: {0}")]
    PacketId(#[source] PacketIdError),
    #[error("Missing payload")]
    MissingPayload,
    #[error("I/O error")]
    IOError(#[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum PacketIdError {
    #[error("Packet id is invalid: {0}")]
    Parse(#[source] VarIntError),
    #[error("There is no status with packet id: {0}")]
    InvalidType(i32),
}

const U64_SIZE_IN_BYTES: usize = 8;

impl Status {
    pub async fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, StatusError> {
        let _length = VarInt::read(cursor).await.map_err(StatusError::Length)?;
        let packet_id = match VarInt::read(cursor).await {
            Ok(VarInt(0x00)) => Ok(StatusPacketId::Status),
            Ok(VarInt(0x01)) => Ok(StatusPacketId::Ping),
            Err(e) => Err(StatusError::PacketId(PacketIdError::Parse(e))),
            Ok(VarInt(n)) => Err(StatusError::PacketId(PacketIdError::InvalidType(n))),
        }?;

        let payload = cursor.read_u64().await.ok();

        Ok(Status { packet_id, payload })
    }

    pub async fn write(&self, writer: &mut WriteHalf<'_>) -> Result<usize, StatusError> {
        let mut response = vec![];
        let packet_id_as_varint = self.packet_id.to_varint();

        match self.packet_id {
            StatusPacketId::Status => {
                // pretend that this is serialized from an actual server status.
                let dummy_json_string = json!({
                "version": {
                    "name": "1.19.4",
                    "protocol": 763
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
                })
                .to_string();

                let string_len = VarInt(dummy_json_string.len() as i32);
                let packet_len = VarInt(
                    (self.packet_id.to_varint().size()
                        + string_len.size()
                        + dummy_json_string.len()) as i32,
                );

                packet_len.write(&mut response).await;
                packet_id_as_varint.write(&mut response).await;

                string_len.write(&mut response).await;
                response.extend_from_slice(dummy_json_string.as_bytes());
            }
            StatusPacketId::Ping => {
                let payload = self.payload.ok_or(StatusError::MissingPayload)?;

                let packet_len = VarInt((packet_id_as_varint.size() + U64_SIZE_IN_BYTES) as i32);

                packet_len.write(&mut response).await;
                packet_id_as_varint.write(&mut response).await;
                response
                    .write_u64(payload)
                    .await
                    .map_err(StatusError::IOError)?;
            }
        }

        writer
            .write_all(&response)
            .await
            .map_err(StatusError::IOError)?;

        Ok(response.len())
    }
}

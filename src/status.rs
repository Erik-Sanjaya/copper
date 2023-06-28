use std::{
    io::{Cursor, Write},
    net::TcpStream,
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use tracing::debug;

use crate::data_types::{VarInt, VarIntError};
use crate::server_status::ServerStatus;
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
    pub fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, StatusError> {
        let _length = VarInt::read(cursor).map_err(StatusError::Length)?;
        let packet_id = match VarInt::read(cursor) {
            Ok(VarInt(0x00)) => Ok(StatusPacketId::Status),
            Ok(VarInt(0x01)) => Ok(StatusPacketId::Ping),
            Err(e) => Err(StatusError::PacketId(PacketIdError::Parse(e))),
            Ok(VarInt(n)) => Err(StatusError::PacketId(PacketIdError::InvalidType(n))),
        }?;

        let payload = cursor.read_u64::<BigEndian>().ok();

        Ok(Status { packet_id, payload })
    }

    pub fn write(&self, writer: &mut TcpStream) -> Result<usize, StatusError> {
        let mut response = vec![];
        let packet_id_as_varint = self.packet_id.to_varint();

        match self.packet_id {
            StatusPacketId::Status => {
                let server_status = ServerStatus::get_example();
                let server_status_as_vec = serde_json::to_vec(&server_status).unwrap();

                let string_len = VarInt(server_status_as_vec.len() as i32);
                let packet_len = VarInt(
                    (self.packet_id.to_varint().size()
                        + string_len.size()
                        + server_status_as_vec.len()) as i32,
                );

                packet_len.write(&mut response);
                packet_id_as_varint.write(&mut response);

                string_len.write(&mut response);
                response.extend_from_slice(server_status_as_vec.as_ref());
            }
            StatusPacketId::Ping => {
                let payload = self.payload.ok_or(StatusError::MissingPayload)?;

                let packet_len = VarInt((packet_id_as_varint.size() + U64_SIZE_IN_BYTES) as i32);

                packet_len.write(&mut response);
                packet_id_as_varint.write(&mut response);
                response
                    .write_u64::<BigEndian>(payload)
                    .map_err(StatusError::IOError)?;
            }
        }

        writer.write_all(&response).map_err(StatusError::IOError)?;

        debug!("Response written: {:?}", response);

        Ok(response.len())
    }
}

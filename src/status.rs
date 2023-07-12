use std::{
    io::{Cursor, Read, Write},
    net::TcpStream,
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use tracing::{debug, error};

use crate::{
    data_types::{DataType, ProtocolString, VarInt},
    ProtocolError,
};
use crate::{packet::ServerBound, server_status::ServerStatus};

pub enum StatusClientBound {
    StatusResponse(StatusResponse),
    PingResponse(PingResponse),
}

impl StatusClientBound {}

impl StatusClientBound {
    pub fn write_to(&self, stream: &mut TcpStream) -> Result<usize, ProtocolError> {
        match self {
            StatusClientBound::StatusResponse(StatusResponse { json_response }) => {
                let mut buffer = vec![];

                let packet_id = VarInt(0);
                let packet_length = VarInt((packet_id.size() + json_response.size()) as i32);

                packet_length.write_to(&mut buffer)?;
                packet_id.write_to(&mut buffer)?;
                json_response.write_to(&mut buffer)?;

                stream.write_all(&buffer)?;

                Ok(buffer.len())
            }
            StatusClientBound::PingResponse(req) => Err(ProtocolError::Unimplemented),
        }
    }

    pub fn from_request(request: ServerBound) -> Result<Self, ProtocolError> {
        match request {
            ServerBound::Status(req) => match req {
                StatusServerBound::StatusRequest(_) => {
                    let server_status = ServerStatus::get_example();
                    let status_string = serde_json::to_string(&server_status)?;

                    Ok(Self::StatusResponse(StatusResponse {
                        json_response: ProtocolString::from(status_string),
                    }))
                }
                StatusServerBound::PingRequest(PingRequest { payload }) => {
                    Ok(Self::PingResponse(PingResponse { payload }))
                }
            },
            _ => {
                error!("why would the request be in another state? should be impossible.");
                Err(ProtocolError::Internal)
            }
        }
    }
}

pub struct StatusResponse {
    json_response: ProtocolString,
}

impl StatusResponse {
    fn from_request(request: StatusServerBound) -> Result<Self, ProtocolError> {
        match request {
            StatusServerBound::StatusRequest(_) => Ok(Self {
                json_response: ProtocolString::from(serde_json::to_string(
                    &ServerStatus::get_example(),
                )?),
            }),
            StatusServerBound::PingRequest(_) => Err(ProtocolError::Malformed),
        }
    }
    fn write_to(&self, stream: &mut TcpStream) -> Result<usize, ProtocolError> {
        let mut response_buffer = vec![];

        let packet_id = VarInt(0);
        let packet_length = VarInt((packet_id.size() + self.json_response.size()) as i32);

        packet_length.write_to(&mut response_buffer)?;
        packet_id.write_to(&mut response_buffer)?;
        self.json_response.write_to(&mut response_buffer)?;

        stream.write_all(&response_buffer)?;

        Ok(response_buffer.len())
    }
}

pub struct PingResponse {
    payload: u64,
}

impl PingResponse {
    fn write_to(&self, stream: &mut TcpStream) -> Result<usize, ProtocolError> {
        Err(ProtocolError::Unimplemented)
    }
}

pub enum StatusServerBound {
    StatusRequest(StatusRequest),
    PingRequest(PingRequest),
}

impl StatusServerBound {
    pub fn read_from(stream: &mut TcpStream) -> Result<Self, ProtocolError> {
        let length = VarInt::read_from(stream)?.0 as usize;

        let packet_id = VarInt::read_from(stream)?;

        let mut buffer = vec![0; length - packet_id.size()];
        stream.read_exact(&mut buffer)?;

        match packet_id {
            VarInt(0x00) => Ok(Self::StatusRequest(StatusRequest::read_from(stream)?)),
            VarInt(0x01) => Ok(Self::PingRequest(PingRequest::read_from(stream)?)),
            VarInt(n) => Err(ProtocolError::PacketId(n)),
        }
    }
}

pub struct StatusRequest {}

impl StatusRequest {
    pub fn read_from<R>(reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        // like, this can't fail. it's just formality
        Ok(Self {})
    }
}

pub struct PingRequest {
    payload: u64,
}

impl PingRequest {
    pub fn read_from<R>(reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        let payload = reader.read_u64::<BigEndian>()?;
        Ok(Self { payload })
    }
}

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

const U64_SIZE_IN_BYTES: usize = 8;

impl Status {
    pub fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, ProtocolError> {
        let packet_id = match VarInt::read_from(cursor) {
            Ok(VarInt(0x00)) => Ok(StatusPacketId::Status),
            Ok(VarInt(0x01)) => Ok(StatusPacketId::Ping),
            Err(e) => Err(e),
            Ok(VarInt(n)) => Err(ProtocolError::PacketId(n)),
        }?;

        // in the case that read_u64 accidentally read false data.
        let payload = match packet_id {
            StatusPacketId::Status => None,
            StatusPacketId::Ping => Some(cursor.read_u64::<BigEndian>()?),
        };

        Ok(Status { packet_id, payload })
    }

    pub fn write(&self, writer: &mut TcpStream) -> Result<usize, ProtocolError> {
        let mut response = vec![];
        let packet_id_as_varint = self.packet_id.to_varint();

        match self.packet_id {
            StatusPacketId::Status => {
                let server_status = ServerStatus::get_example();
                let status_as_protocol_string =
                    ProtocolString::from(serde_json::to_string(&server_status).unwrap());

                let status_entire_length = status_as_protocol_string.length.0
                    + status_as_protocol_string.length.size() as i32;
                let packet_len =
                    VarInt(self.packet_id.to_varint().size() as i32 + status_entire_length);

                packet_len.write_to(&mut response)?;
                packet_id_as_varint.write_to(&mut response)?;

                status_as_protocol_string.write_to(&mut response)?;
            }
            StatusPacketId::Ping => {
                let payload = self.payload.ok_or(ProtocolError::Missing)?;

                let packet_len = VarInt((packet_id_as_varint.size() + U64_SIZE_IN_BYTES) as i32);

                packet_len.write_to(&mut response)?;
                packet_id_as_varint.write_to(&mut response)?;
                response.write_u64::<BigEndian>(payload)?;
            }
        }

        writer.write_all(&response)?;

        debug!("Response written: {:?}", response);

        Ok(response.len())
    }
}

use std::io::{Cursor, Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use tracing::{error, trace};

use crate::{
    data_types::{DataType, ProtocolString, VarInt},
    packet::{Decodable, Encodable},
    ProtocolError,
};
use crate::{packet, server_status::ServerStatus};

#[derive(Debug)]
pub enum ClientBound {
    StatusResponse(StatusResponse),
    PingResponse(PingResponse),
}

impl Encodable for ClientBound {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<usize, ProtocolError> {
        match self {
            Self::StatusResponse(StatusResponse { json_response }) => {
                let mut buffer = vec![];

                let packet_id = VarInt(0x00);
                let packet_length = VarInt(i32::try_from(packet_id.size() + json_response.size())?);

                packet_length.write_to(&mut buffer)?;
                packet_id.write_to(&mut buffer)?;
                json_response.write_to(&mut buffer)?;

                writer.write_all(&buffer)?;

                Ok(buffer.len())
            }
            Self::PingResponse(res) => {
                let mut buffer = vec![];

                let packet_id = VarInt(0x01);
                let packet_length = VarInt(i32::try_from(packet_id.size() + U64_SIZE_IN_BYTES)?);

                packet_length.write_to(&mut buffer)?;
                packet_id.write_to(&mut buffer)?;
                res.write_to(&mut buffer)?;

                writer.write_all(&buffer)?;

                Ok(buffer.len())
            }
        }
    }
}

impl ClientBound {
    pub fn from_request(request: ServerBound) -> Result<Self, ProtocolError> {
        match request {
            ServerBound::StatusRequest(_) => {
                let server_status = ServerStatus::get_example();
                let status_string = serde_json::to_string(&server_status)?;

                Ok(Self::StatusResponse(StatusResponse {
                    json_response: ProtocolString::try_from(status_string)?,
                }))
            }
            ServerBound::PingRequest(PingRequest { payload }) => {
                Ok(Self::PingResponse(PingResponse { payload }))
            }
        }
    }
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct StatusResponse {
    json_response: ProtocolString,
}

impl StatusResponse {
    // fn from_request(request: ServerBound) -> Result<Self, ProtocolError> {
    //     match request {
    //         ServerBound::StatusRequest(_) => Ok(Self {
    //             json_response: ProtocolString::try_from(serde_json::to_string(
    //                 &ServerStatus::get_example(),
    //             )?)?,
    //         }),
    //         ServerBound::PingRequest(_) => Err(ProtocolError::Malformed),
    //     }
    // }

    // fn write_to(&self, stream: &mut TcpStream) -> Result<usize, ProtocolError> {
    //     let mut response_buffer = vec![];

    //     let packet_id = VarInt(0);
    //     let packet_length = VarInt(i32::try_from(packet_id.size() + self.json_response.size())?);

    //     packet_length.write_to(&mut response_buffer)?;
    //     packet_id.write_to(&mut response_buffer)?;
    //     self.json_response.write_to(&mut response_buffer)?;

    //     stream.write_all(&response_buffer)?;

    //     Ok(response_buffer.len())
    // }
}

#[derive(Debug)]
pub struct PingResponse {
    payload: u64,
}

impl PingResponse {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<usize, ProtocolError> {
        let payload = self.payload;
        writer.write_u64::<BigEndian>(payload)?;

        Ok(U64_SIZE_IN_BYTES)
    }
}

#[derive(Debug)]
pub enum ServerBound {
    StatusRequest(StatusRequest),
    PingRequest(PingRequest),
}

impl Decodable for ServerBound {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let length = VarInt::read_from(reader)?;
        let length = usize::try_from(length.0)?;

        let packet_id = VarInt::read_from(reader)?;
        trace!("Status Packet ID: {:?}", packet_id);

        let mut buffer = vec![0; length - packet_id.size()];
        reader.read_exact(&mut buffer)?;
        trace!("Buffer: {:?}", buffer);

        let mut cursor = Cursor::new(&mut buffer);

        match packet_id {
            VarInt(0x00) => Ok(Self::StatusRequest(StatusRequest::read_from(&mut cursor)?)),
            VarInt(0x01) => Ok(Self::PingRequest(PingRequest::read_from(&mut cursor)?)),
            VarInt(n) => Err(ProtocolError::PacketId(n)),
        }
    }
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct StatusRequest;

impl Decodable for StatusRequest {
    fn read_from<R>(_reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        Ok(Self)
    }
}

#[derive(Debug)]
pub struct PingRequest {
    payload: u64,
}

impl Decodable for PingRequest {
    fn read_from<R>(reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        let payload = reader.read_u64::<BigEndian>()?;
        trace!("payload: {}", payload);
        Ok(Self { payload })
    }
}

#[derive(Debug)]
pub struct Status {
    pub packet_id: PacketId,
    pub payload: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PacketId {
    Status = 0x00,
    Ping = 0x01,
}

impl PacketId {
    const fn to_varint(&self) -> VarInt {
        match self {
            Self::Status => VarInt(0x00),
            Self::Ping => VarInt(0x01),
        }
    }
}

const U64_SIZE_IN_BYTES: usize = 8;

// impl Status {
//     pub fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, ProtocolError> {
//         let packet_id = match VarInt::read_from(cursor) {
//             Ok(VarInt(0x00)) => Ok(StatusPacketId::Status),
//             Ok(VarInt(0x01)) => Ok(StatusPacketId::Ping),
//             Err(e) => Err(e),
//             Ok(VarInt(n)) => Err(ProtocolError::PacketId(n)),
//         }?;

//         // in the case that read_u64 accidentally read false data.
//         let payload = match packet_id {
//             StatusPacketId::Status => None,
//             StatusPacketId::Ping => Some(cursor.read_u64::<BigEndian>()?),
//         };

//         Ok(Self { packet_id, payload })
//     }

//     pub fn write(&self, writer: &mut TcpStream) -> Result<usize, ProtocolError> {
//         let mut response = vec![];
//         let packet_id_as_varint = self.packet_id.to_varint();

//         match self.packet_id {
//             StatusPacketId::Status => {
//                 let server_status = ServerStatus::get_example();
//                 let status_as_protocol_string =
//                     ProtocolString::try_from(serde_json::to_string(&server_status)?)?;

//                 let status_entire_length = status_as_protocol_string.length.0
//                     + i32::try_from(status_as_protocol_string.length.size())?;
//                 let packet_len = VarInt(
//                     i32::try_from(self.packet_id.to_varint().size())? + status_entire_length,
//                 );

//                 packet_len.write_to(&mut response)?;
//                 packet_id_as_varint.write_to(&mut response)?;

//                 status_as_protocol_string.write_to(&mut response)?;
//             }
//             StatusPacketId::Ping => {
//                 let payload = self.payload.ok_or(ProtocolError::Missing)?;

//                 let packet_len = VarInt(i32::try_from(
//                     packet_id_as_varint.size() + U64_SIZE_IN_BYTES,
//                 )?);

//                 packet_len.write_to(&mut response)?;
//                 packet_id_as_varint.write_to(&mut response)?;
//                 response.write_u64::<BigEndian>(payload)?;
//             }
//         }

//         writer.write_all(&response)?;

//         debug!("Response written: {:?}", response);

//         Ok(response.len())
//     }
// }

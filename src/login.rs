use std::io::{Cursor, Read, Write};

use byteorder::ReadBytesExt;

use tracing::trace;
use uuid::Uuid;

use crate::{
    data_types::{DataType, ProtocolString, VarInt},
    packet::Decodable,
    ProtocolError,
};

// 1. C→S: Handshake with Next State set to 2 (login)
// 2. C→S: Login Start
// 3. S→C: Encryption Request
// 4. Client auth
// 5. C→S: Encryption Response
// 6. Server auth, both enable encryption
// 7. S→C: Set Compression (optional)
// 8. S→C: Login Success

#[derive(Debug)]
pub enum ClientBound {
    Disconnect(Disconnect),
    EncryptionRequest(EncryptionRequest),
    LoginSuccess(LoginSuccess),
    SetCompression(SetCompression),
    LoginPluginRequest(PluginRequest),
}

impl ClientBound {
    pub fn write_to<W>(&self, writer: &mut W) -> Result<usize, ProtocolError>
    where
        W: Write,
    {
        match self {
            Self::Disconnect(packet) => packet.write_to(writer),
            Self::EncryptionRequest(_packet) => {
                trace!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }
            Self::LoginSuccess(packet) => packet.write_to(writer),
            Self::SetCompression(_packet) => {
                trace!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }
            Self::LoginPluginRequest(_packet) => {
                trace!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }
        }
    }

    pub fn from_request(request: ServerBound) -> Result<Self, ProtocolError> {
        match request {
            ServerBound::LoginStart(req) => Ok(Self::LoginSuccess(LoginSuccess {
                // TODO dont do this
                uuid: req.player_uuid.unwrap_or_default(),
                username: req.name,
                number_of_properties: VarInt(0),
                property: vec![],
            })),
            _ => Err(ProtocolError::Unimplemented),
        }
    }
}

#[derive(Debug)]
pub struct Disconnect {
    reason: ProtocolString,
}

impl Disconnect {
    pub const fn new(reason: ProtocolString) -> Self {
        Self { reason }
    }

    fn write_to<W>(&self, writer: &mut W) -> Result<usize, ProtocolError>
    where
        W: Write,
    {
        let mut response = vec![];
        let packet_id = VarInt(0x00);
        let packet_length = VarInt(i32::try_from(packet_id.size() + self.reason.size())?);

        packet_length.write_to(&mut response)?;
        packet_id.write_to(&mut response)?;
        self.reason.write_to(&mut response)?;

        writer.write_all(&response)?;

        Ok(response.len())
    }
}

#[derive(Debug)]
pub struct EncryptionRequest {
    server_id: String,
    public_key_length: VarInt,
    public_key: Vec<u8>,
    verify_token_length: VarInt,
    verify_token: Vec<u8>,
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct LoginSuccess {
    pub uuid: Uuid,
    pub username: ProtocolString,
    pub number_of_properties: VarInt,
    // the fields below are part of an "array" labelled property. i don't know
    // how i'm supposed to represent the array in the stream yet, so i'll just
    // leave it like this for now. hopefully before the next commit i can delete
    // this comment
    property: Vec<Property>,
}

#[derive(Debug)]
struct Property {
    name: ProtocolString,
    value: ProtocolString,
    is_signed: bool,
    signature: ProtocolString,
}

impl LoginSuccess {
    fn write_to<W>(&self, writer: &mut W) -> Result<usize, ProtocolError>
    where
        W: Write,
    {
        if !self.property.is_empty() {
            trace!("unimplemented");
            return Err(ProtocolError::Unimplemented);
        }

        let mut response = vec![];
        let packet_id = VarInt(0x02);
        let uuid = Uuid::as_bytes(&self.uuid);
        let packet_length = VarInt(i32::try_from(
            packet_id.size() + uuid.len() + self.username.size() + self.number_of_properties.size(),
        )?);

        packet_length.write_to(&mut response)?;
        packet_id.write_to(&mut response)?;
        response.extend_from_slice(uuid);
        self.username.write_to(&mut response)?;
        self.number_of_properties.write_to(&mut response)?;

        // TODO: write the rest. i'm still unsure as to how it works

        writer.write_all(&response)?;

        Ok(response.len())
    }
}

#[derive(Debug)]
pub struct SetCompression {
    threshold: VarInt,
}

#[derive(Debug)]
pub struct PluginRequest {
    message_id: VarInt,
    channel: ProtocolString,
    data: Vec<u8>,
}

#[derive(Debug)]
pub enum ServerBound {
    LoginStart(LoginStart),
    EncryptionResponse(EncryptionResponse),
    LoginPluginResponse(LoginPluginResponse),
}

impl Decodable for ServerBound {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let length = VarInt::read_from(reader)?;
        let length = usize::try_from(length.0)?;

        let packet_id = VarInt::read_from(reader)?;
        trace!("Login Packet ID: {:?}", packet_id);

        let mut buffer = vec![0; length - packet_id.size()];
        reader.read_exact(&mut buffer)?;
        trace!("Buffer: {:?}", buffer);

        let mut cursor = Cursor::new(&mut buffer);

        match packet_id {
            VarInt(0x00) => Ok(Self::LoginStart(LoginStart::read_from(&mut cursor)?)),
            VarInt(0x01) => {
                trace!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }
            VarInt(0x02) => {
                trace!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }

            VarInt(n) => Err(ProtocolError::PacketId(n)),
        }
    }
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct LoginStart {
    pub name: ProtocolString,
    pub has_player_uuid: bool,
    pub player_uuid: Option<Uuid>,
}

impl LoginStart {
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let name = ProtocolString::read_from(reader)?;
        let has_player_uuid = reader.read_u8()? != 0;

        let player_uuid = {
            let mut rest_of_bytes = vec![];
            match reader.read_to_end(&mut rest_of_bytes)? {
                0 => None,
                16 => {
                    let buffer_for_uuid = rest_of_bytes[..16]
                        .try_into()
                        .map_err(|_| ProtocolError::Parsing)?;

                    Some(Uuid::from_bytes(buffer_for_uuid))
                }
                _ => return Err(ProtocolError::Malformed),
            }
        };

        if has_player_uuid && player_uuid.is_none() || !has_player_uuid && player_uuid.is_some() {
            return Err(ProtocolError::Malformed);
        }

        Ok(Self {
            name,
            has_player_uuid,
            player_uuid,
        })
    }
}

#[derive(Debug)]
pub struct EncryptionResponse {
    shared_secret_length: VarInt,
    shared_secret: Vec<u8>,
    verify_token_length: VarInt,
    verify_token: Vec<u8>,
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct LoginPluginResponse {
    message_id: VarInt,
    successful: bool,
    data: Option<Vec<u8>>,
}

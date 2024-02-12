use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use tracing::trace;

use crate::{
    data_types::{DataType, ProtocolString, VarInt},
    packet::Decodable,
    ProtocolError, State,
};

#[derive(Debug)]
pub enum ServerBound {
    Handshake(Handshake),
    // TODO
    Legacy(Legacy),
}

impl Decodable for ServerBound {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let VarInt(length) = VarInt::read_from(reader)?;
        let length = usize::try_from(length)?;

        let packet_id = VarInt::read_from(reader)?;

        trace!("Handshaking Packet ID: {:?}", packet_id);

        let buffer_size = length - packet_id.size();
        trace!("Buffer Size: {}", buffer_size);
        let mut buffer = vec![0; buffer_size];

        reader.read_exact(&mut buffer[..])?;
        trace!("Buffer: {:?}", buffer);

        if buffer.len() != buffer_size {
            return Err(ProtocolError::Malformed);
        }

        let mut cursor = Cursor::new(buffer);

        match packet_id {
            VarInt(0x00) => Ok(Self::Handshake(Handshake::read_from(&mut cursor)?)),
            VarInt(0xFE) => Ok(Self::Legacy(Legacy::read_from(&mut cursor)?)),
            VarInt(n) => Err(ProtocolError::PacketId(n)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NextState {
    Status = 1,
    Login = 2,
}

#[derive(Debug)]
pub struct Handshake {
    protocol_version: VarInt,
    server_address: ProtocolString,
    server_port: u16,
    next_state: NextState,
}

impl Handshake {
    fn read_from<R>(reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        let protocol_version = VarInt::read_from(reader)?;
        trace!("Protocol Version: {:?}", protocol_version);
        let server_address = ProtocolString::read_from(reader)?;
        trace!("Server Address: {:?}", protocol_version);
        let server_port = reader.read_u16::<BigEndian>()?;
        trace!("Server Port: {:?}", protocol_version);
        let next_state = match VarInt::read_from(reader)? {
            VarInt(1) => NextState::Status,
            VarInt(2) => NextState::Login,
            VarInt(_) => return Err(ProtocolError::Malformed),
        };

        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }

    pub const fn get_next_state(&self) -> State {
        match self.next_state {
            NextState::Status => State::Status,
            NextState::Login => State::Login,
        }
    }
}

#[derive(Debug)]
pub struct Legacy {
    pub payload: u8,
}

impl Legacy {
    fn read_from<R>(reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        let payload = reader.read_u8()?;

        Ok(Self { payload })
    }
}

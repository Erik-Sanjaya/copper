use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use tracing::trace;

use crate::{
    data_types::{DataType, ProtocolString, VarInt},
    ProtocolError,
};

#[derive(Debug)]
pub enum HandshakingServerBound {
    Handshake(Handshake),
    // TODO
    Legacy,
}

impl HandshakingServerBound {
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let VarInt(length) = VarInt::read_from(reader)?;
        let length = length as usize;

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
            VarInt(0x00) => Ok(HandshakingServerBound::Handshake(Handshake::read_from(
                &mut cursor,
            )?)),
            VarInt(0xFE) => Ok(HandshakingServerBound::Handshake(Handshake::legacy(
                &mut cursor,
            )?)),
            VarInt(n) => Err(ProtocolError::PacketId(n)),
        }
    }
}

#[derive(Debug)]
pub struct Handshake {
    protocol_version: VarInt,
    server_address: ProtocolString,
    server_port: u16,
    get_next_state: HandshakingNextState,
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
            VarInt(1) => HandshakingNextState::Status,
            VarInt(2) => HandshakingNextState::Login,
            VarInt(_) => return Err(ProtocolError::Malformed),
        };

        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            get_next_state: next_state,
        })
    }

    fn legacy<R>(_reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        unimplemented!()
    }

    pub fn get_next_state(&self) -> HandshakingNextState {
        return self.get_next_state.clone();
    }
}

#[derive(Debug)]
pub struct Handshaking {
    packet_id: VarInt,
    protocol_version: VarInt,
    server_address: String,
    server_port: u16,
    next_state: HandshakingNextState,
}

#[derive(Debug, Clone)]
pub enum HandshakingNextState {
    Status = 1,
    Login = 2,
}

impl Handshaking {
    pub fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, ProtocolError> {
        let packet_id = VarInt::read_from(cursor)?;

        let protocol_version = VarInt::read_from(cursor)?;

        let server_address = {
            let server_addr_len = VarInt::read_from(cursor)?;
            let mut server_addr_buffer = vec![0; server_addr_len.0 as usize];
            cursor.read_exact(&mut server_addr_buffer)?;

            String::from_utf8(server_addr_buffer).map_err(|_| ProtocolError::Malformed)?
        };

        let server_port = cursor.read_u16::<BigEndian>()?;

        let next_state = match VarInt::read_from(cursor)? {
            VarInt(1) => HandshakingNextState::Status,
            VarInt(2) => HandshakingNextState::Login,
            VarInt(_) => return Err(ProtocolError::Malformed),
        };

        Ok(Handshaking {
            packet_id,
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }
}

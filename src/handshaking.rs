use std::{
    io::{Cursor, Read},
    net::TcpStream,
};

use byteorder::{BigEndian, ReadBytesExt};

use crate::{
    data_types::{DataType, ProtocolString, VarInt},
    ProtocolError,
};

#[derive(Debug)]
pub enum HandshakingServerBound {
    Handshake(Handshake),
}

impl HandshakingServerBound {
    pub fn read_from(stream: &mut TcpStream) -> Result<Self, ProtocolError> {
        let length = VarInt::read_from(stream)?.0 as usize;

        let packet_id = VarInt::read_from(stream)?;

        let mut buffer = vec![0; length - packet_id.size()];
        stream.read_exact(&mut buffer)?;

        let mut cursor = Cursor::new(buffer);

        match packet_id {
            VarInt(0x00) => Ok(HandshakingServerBound::Handshake(Handshake::read_from(
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
    next_state: HandshakingNextState,
}

impl Handshake {
    fn read_from<R>(reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        let protocol_version = VarInt::read_from(reader)?;
        let server_address = ProtocolString::read_from(reader)?;
        let server_port = reader.read_u16::<BigEndian>()?;
        let next_state = match VarInt::read_from(reader)? {
            VarInt(1) => HandshakingNextState::Status,
            VarInt(2) => HandshakingNextState::Login,
            VarInt(_) => return Err(ProtocolError::Malformed),
        };

        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }
}

#[derive(Debug)]
pub struct Handshaking {
    packet_id: VarInt,
    protocol_version: VarInt,
    server_address: String,
    server_port: u16,
    pub next_state: HandshakingNextState,
}

#[derive(Debug)]
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

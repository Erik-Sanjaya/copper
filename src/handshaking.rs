use std::{
    io::{Cursor, Read},
    string::FromUtf8Error,
};

use byteorder::{BigEndian, ReadBytesExt};
use thiserror::Error;

use crate::{
    data_types::{VarInt, VarIntError},
    ProtocolError,
};

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

#[derive(Debug, Error)]
pub enum NextStateError {
    #[error("Packet length is invalid: {0}")]
    Parse(#[source] VarIntError),
    #[error("There is no next state option: {0}")]
    InvalidType(i32),
}

impl Handshaking {
    pub fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, ProtocolError> {
        let packet_id = VarInt::read_from(cursor)?;

        let protocol_version = VarInt::read_from(cursor)?;

        let server_address = {
            let server_addr_len = VarInt::read_from(cursor)?;
            let mut server_addr_buffer = vec![0; server_addr_len.0 as usize];
            cursor
                .read_exact(&mut server_addr_buffer)
                .map_err(ProtocolError::IOError)?;

            String::from_utf8(server_addr_buffer).map_err(|_| ProtocolError::Malformed)?
        };

        let server_port = cursor
            .read_u16::<BigEndian>()
            .map_err(ProtocolError::IOError)?;

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

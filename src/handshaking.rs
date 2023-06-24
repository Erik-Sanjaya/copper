use std::{io::Cursor, string::FromUtf8Error};

use thiserror::Error;
use tokio::io::AsyncReadExt;

use crate::data_types::{VarInt, VarIntError};

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
pub enum HandshakingError {
    #[error("Packet length is invalid: {0}")]
    Length(#[source] VarIntError),
    #[error("Packet id is invalid: {0}")]
    PacketId(#[source] VarIntError),
    #[error("Protocol version is invalid: {0}")]
    ProtocolVersion(#[source] VarIntError),
    #[error("Error with server address: {0}")]
    ServerAddress(#[source] ServerAddressError),
    #[error("Server port is invalid: {0}")]
    ServerPort(#[source] std::io::Error),
    #[error("Error with next state: {0}")]
    NextState(#[source] NextStateError),
}

#[derive(Debug, Error)]
pub enum ServerAddressError {
    #[error("Invalid server address length: {0}")]
    Length(#[source] VarIntError),
    #[error("Server address is missing bytes: {0}")]
    MissingBytes(#[source] std::io::Error),
    #[error("Server address is invalid: {0}")]
    Parsing(#[source] FromUtf8Error),
}

#[derive(Debug, Error)]
pub enum NextStateError {
    #[error("Packet length is invalid: {0}")]
    Parse(#[source] VarIntError),
    #[error("There is no next state option: {0}")]
    InvalidType(i32),
}

impl Handshaking {
    pub async fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, HandshakingError> {
        let _length = VarInt::read(cursor)
            .await
            .map_err(HandshakingError::Length)?;

        let packet_id = VarInt::read(cursor)
            .await
            .map_err(HandshakingError::PacketId)?;

        let protocol_version = VarInt::read(cursor)
            .await
            .map_err(HandshakingError::ProtocolVersion)?;

        let server_address = {
            let server_addr_len = VarInt::read(cursor)
                .await
                .map_err(ServerAddressError::Length)
                .map_err(HandshakingError::ServerAddress)?;
            let mut server_addr_buffer = vec![0; server_addr_len.0 as usize];
            cursor
                .read_exact(&mut server_addr_buffer)
                .await
                .map_err(ServerAddressError::MissingBytes)
                .map_err(HandshakingError::ServerAddress)?;

            String::from_utf8(server_addr_buffer)
                .map_err(ServerAddressError::Parsing)
                .map_err(HandshakingError::ServerAddress)?
        };

        let server_port = cursor
            .read_u16()
            .await
            .map_err(HandshakingError::ServerPort)?;

        let next_state = match VarInt::read(cursor)
            .await
            .map_err(NextStateError::Parse)
            .map_err(HandshakingError::NextState)?
        {
            VarInt(1) => HandshakingNextState::Status,
            VarInt(2) => HandshakingNextState::Login,
            VarInt(n) => return Err(HandshakingError::NextState(NextStateError::InvalidType(n))),
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

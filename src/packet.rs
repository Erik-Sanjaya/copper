use std::{
    io::{Read, Write},
    net::TcpStream,
};

use tracing::{debug, error, trace};

use crate::{
    data_types::{DataType, VarInt},
    handshaking::HandshakingServerBound,
    login::{LoginClientBound, LoginServerBound},
    play::{PlayClientBound, PlayServerBound},
    status::{StatusClientBound, StatusServerBound},
    ProtocolError, State,
};

trait PacketClientBound {
    fn write_to<W: Write>(&self, writer: W) -> Result<usize, ProtocolError>;
}

trait PacketServerBound: Sized {
    fn read_from<R: Read>(reader: R) -> Result<Self, ProtocolError>;
}

struct Packet(Vec<u8>);
// TODO list
// - trait for writing and reading from and to stream
// - good naming, i don't know which one to call packet, which one to call protocol, etc etc.
// - impl From<Packet> for ... i guess

// ! this probably wont be used for a while, as i can't yet see the shape i want
// ! these stuff to take. in fact, it might be not used at all and replaced by
// ! something else later on.

impl Packet {
    pub fn read_stream(stream: &mut TcpStream, state: &State) -> Result<Self, ProtocolError> {
        let length = VarInt::read_from(stream)?;
        if length.0 == 0 {
            trace!("length is 0, most likely EOF");
            return Err(ProtocolError::IOError(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "length of packet is 0",
            )));
        }

        // for my beloved, legacy stuff
        // modern handshake shouldn't be 0xFE long, so this should be good enough of a check
        if length.0 == 0xFE && state == &State::Handshaking {
            trace!("unimplemented");
            return Err(ProtocolError::Unimplemented);
        }

        let mut buffer = vec![0; length.0 as usize];
        match stream.read_exact(&mut buffer[..]) {
            Ok(_) => (),
            Err(e) => {
                error!("Error with reading_exact: {:?}", e);
                debug!("Buffer length: {:?}", length);
                debug!("Buffer: {:?}", buffer);
                return Err(ProtocolError::IOError(e));
            }
        };

        Ok(Self(buffer))
    }

    pub fn write_stream(stream: &mut TcpStream) -> Result<usize, ProtocolError> {
        Err(ProtocolError::Unimplemented)
    }
}

pub enum ClientBound {
    Status(StatusClientBound),
    Login(LoginClientBound),
    Play(PlayClientBound),
}

impl ClientBound {
    pub fn parse_packet(
        stream: &mut TcpStream,
        state: &State,
        request: ServerBound,
    ) -> Result<Self, ProtocolError> {
        match state {
            State::Handshaking => {
                error!("Handshaking packet for clientbound?");
                Err(ProtocolError::Internal)
            }
            State::Status => Ok(Self::Status(StatusClientBound::from_request(request)?)),
            State::Login => Err(ProtocolError::Unimplemented),
            State::Play => Err(ProtocolError::Unimplemented),
        }
    }

    pub fn write_to(&self, stream: &mut TcpStream) -> Result<usize, ProtocolError> {
        Err(ProtocolError::Unimplemented)
    }
}

pub enum ServerBound {
    Handshake(HandshakingServerBound),
    Status(StatusServerBound),
    Login(LoginServerBound),
    Play(PlayServerBound),
}

impl ServerBound {
    pub fn parse_packet<R: Read>(reader: &mut R, state: &State) -> Result<Self, ProtocolError> {
        match state {
            State::Handshaking => Ok(Self::Handshake(HandshakingServerBound::read_from(reader)?)),
            State::Status => Ok(Self::Status(StatusServerBound::read_from(reader)?)),
            State::Login => {
                trace!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }
            State::Play => {
                trace!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }
        }
    }
}

//! The module handling all packets
//!
//! This is where the magic does not happen, but the pain does.
//! If you are unfamiliar, the Minecraft protocol is split into 4 states,
//! being `Handshake`, `Status`, `Login`, and `Play`
use std::io::{Cursor, Read, Write};

use tracing::error;

use crate::{handshaking, login, play, status, ProtocolError, State};

pub trait Encodable {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<usize, ProtocolError>;
}

pub trait Decodable: Sized {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError>;
}

// #[derive(Debug)]
// struct Packet(Vec<u8>);
// // TODO list
// // - trait for writing and reading from and to stream
// // - good naming, i don't know which one to call packet, which one to call protocol, etc etc.
// // - impl From<Packet> for ... i guess

// // ! this probably wont be used for a while, as i can't yet see the shape i want
// // ! these stuff to take. in fact, it might be not used at all and replaced by
// // ! something else later on.

// impl Packet {
//     pub fn read_stream(stream: &mut TcpStream, state: &State) -> Result<Self, ProtocolError> {
//         let VarInt(length) = VarInt::read_from(stream)?;
//         if length == 0 {
//             trace!("length is 0, most likely EOF");
//             return Err(ProtocolError::IOError(std::io::Error::new(
//                 std::io::ErrorKind::UnexpectedEof,
//                 "length of packet is 0",
//             )));
//         }

//         // for my beloved, legacy stuff
//         // modern handshake shouldn't be 0xFE long, so this should be good enough of a check
//         if length == 0xFE && state == &State::Handshaking {
//             trace!("unimplemented");
//             return Err(ProtocolError::Unimplemented);
//         }

//         let mut buffer = vec![0; length as usize];
//         match stream.read_exact(&mut buffer[..]) {
//             Ok(_) => (),
//             Err(e) => {
//                 error!("Error with reading_exact: {:?}", e);
//                 debug!("Buffer length: {:?}", length);
//                 debug!("Buffer: {:?}", buffer);
//                 return Err(ProtocolError::IOError(e));
//             }
//         };

//         Ok(Self(buffer))
//     }

//     pub fn write_stream(_stream: &mut TcpStream) -> Result<usize, ProtocolError> {
//         Err(ProtocolError::Unimplemented)
//     }
// }

#[derive(Debug)]
pub enum ClientBound {
    Status(status::ClientBound),
    Login(login::ClientBound),
    Play(play::ClientBound),
}

impl ClientBound {
    pub fn create_reply(
        // is this even needed?
        // stream: &mut tokio::net::TcpStream,
        state: &State,
        request: ServerBound,
    ) -> Result<Self, ProtocolError> {
        match request {
            ServerBound::Handshake(req) => {
                error!("Handshaking packet for clientbound?");
                Err(ProtocolError::Internal)
            }
            ServerBound::Status(req) => Ok(Self::Status(status::ClientBound::from_request(req)?)),
            ServerBound::Login(req) => Ok(Self::Login(login::ClientBound::from_request(req)?)),
            ServerBound::Play(req) => Ok(Self::Play(play::ClientBound::from_request(req)?)),
        }

        // match state {
        //     State::Handshaking => {
        //         error!("Handshaking packet for clientbound?");
        //         Err(ProtocolError::Internal)
        //     }
        //     State::Status => Ok(Self::Status(status::ClientBound::from_request(request)?)),
        //     State::Login => Ok(Self::Login(login::ClientBound::from_request(request)?)),
        //     State::Play => Err(ProtocolError::Unimplemented),
        // }
    }

    // pub fn write_to(&self, stream: &mut tokio::net::TcpStream) -> Result<usize, ProtocolError> {
    //     let mut reply_bytes: Vec<u8> = vec![];
    //     let mut cursor = Cursor::new(&mut reply_bytes);

    //     match self {
    //         Self::Status(res) => res.write_to(&mut cursor),
    //         Self::Login(_) => todo!(),
    //         Self::Play(_) => todo!(),
    //     }?;

    //     Ok(stream.try_write(&reply_bytes)?)
    // }

    pub fn encode(self) -> Result<Vec<u8>, ProtocolError> {
        let mut encoded_packet: Vec<u8> = vec![];
        let mut cursor = Cursor::new(&mut encoded_packet);

        match self {
            Self::Status(res) => res.write_to(&mut cursor),
            Self::Login(res) => res.write_to(&mut cursor),
            Self::Play(_) => todo!(),
        }?;

        Ok(encoded_packet)
    }
}

/// `ServerBound` represents the states in which the packet is in.
#[derive(Debug)]
pub enum ServerBound {
    Handshake(handshaking::ServerBound),
    Status(status::ServerBound),
    Login(login::ServerBound),
    Play(play::ServerBound),
}

impl ServerBound {
    pub fn parse_packet<R: Read>(reader: &mut R, state: &State) -> Result<Self, ProtocolError> {
        match state {
            State::Handshaking => Ok(Self::Handshake(handshaking::ServerBound::read_from(
                reader,
            )?)),
            State::Status => Ok(Self::Status(status::ServerBound::read_from(reader)?)),
            State::Login => Ok(Self::Login(login::ServerBound::read_from(reader)?)),
            State::Play => {
                error!("unimplemented");
                Err(ProtocolError::Unimplemented)
            }
        }
    }
}

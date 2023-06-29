use std::{io::Read, net::TcpStream};

use tracing::{debug, error, trace};

use crate::{data_types::VarInt, ProtocolError, State};

struct Packet(Vec<u8>);
// TODO list
// - trait for writing and reading from and to stream
// - good naming, i don't know which one to call packet, which one to call protocol, etc etc.
// - impl From<Packet> for ... i guess

impl Packet {
    pub fn read_stream(stream: &mut TcpStream, state: &State) -> Result<Self, ProtocolError> {
        let length = VarInt::read_from(stream)?;
        if length.0 == 0 {
            trace!("length is 0, most likely EOF");
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "length of packet is 0",
            ))
            .map_err(ProtocolError::IOError);
        }

        // for my beloved, legacy stuff
        // modern handshake shouldn't be 0xFE long, so this should be good enough of a check
        if length.0 == 0xFE && state == &State::Handshaking {
            return Err(ProtocolError::Unimplemented);
        }

        let mut buffer = vec![0; length.0 as usize];
        match stream.read_exact(&mut buffer[..]) {
            Ok(_) => (),
            Err(e) => {
                error!("Error with reading_exact: {:?}", e);
                debug!("Buffer length: {:?}", length);
                debug!("Buffer: {:?}", buffer);
                return Err(e).map_err(ProtocolError::IOError);
            }
        };

        Ok(Self(buffer))
    }
}

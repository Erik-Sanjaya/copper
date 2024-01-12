use std::{io::Cursor, net::SocketAddr};

use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tracing::trace;

// use crate::data_types::{DataType, VarInt};
use crate::packet::ServerBound;
use crate::ProtocolError;
use crate::State;

pub struct Client {
    addr: SocketAddr,
    stream: TcpStream,
    state: State,
    buffer: Vec<u8>,
}

impl Client {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self {
            addr,
            stream,
            state: State::Handshaking,
            buffer: Vec::new(),
        }
    }

    // i think this one is allowed to consume the client
    // all the calls to the other methods come from this
    // one anyway
    pub async fn handle(&mut self) {
        loop {
            // basically, have the thing be drained
            // then it should be yielding once it empties out.
            // at this stage you already be writing a response packet to the client
            // that way the program wont just keep on yielding the drain
            // and have nothing else to do

            // tokio::select! {
            //     _ = self.drain_stream() => {
            //        // self.read_stream().await; 
            //     }

                
            }
            trace!("BUFFER {:?}", self.buffer);

            if !self.buffer.is_empty() {
                match self.read_stream().await {
                    Ok(packet) => match packet {
                        ServerBound::Handshake(req) => unimplemented!(),
                        ServerBound::Status(req) => unimplemented!(),
                        ServerBound::Login(req) => unimplemented!(),
                        ServerBound::Play(req) => unimplemented!(),
                    },
                    Err(e) => panic!(),
                }
            }
        }
    }

    // async fn parse_packet(&mut self) -> Result<ServerBound, ProtocolError> {
    //     let mut length_buffer = [0; 5];

    //     // honestly, this peeking thing is only done because of the legacy ping
    //     // server list that has no length at all.
    //     // i don't like this double read on the length and have it be unused, so
    //     // i'll probably just hard code it here later.
    //     self.stream.peek(&mut length_buffer[..]).await;
    //     let mut length_cursor = Cursor::new(length_buffer.as_slice());
    //     let length = VarInt::read_from(&mut length_cursor)?;
    //     let mut packet = vec![0; length.size() + length.0 as usize];

    //     self.stream.read_exact(&mut packet).await?;

    //     let mut cursor = Cursor::new(packet.as_slice());
    //     ServerBound::parse_packet(&mut cursor, &self.state)
    // }

    // fn parse_packet(&self, packet: &[u8]) -> Result<ServerBound, ProtocolError> {
    //     let mut cursor = Cursor::new(packet);
    //     ServerBound::parse_packet(&mut cursor, &self.state)
    // }

    pub async fn read_stream(&mut self) -> Result<ServerBound, ProtocolError> {
        let mut temp_cursor = Cursor::new(&self.buffer);
        ServerBound::parse_packet(&mut temp_cursor, &self.state)
    }

    /// Drain the whole stream and move it to the client's internal buffer
    async fn drain_stream(&mut self) -> Result<(), std::io::Error> {
        loop {
            // trace!("LOOP")
            let mut buffer = [0; 128];

            trace!("BUFFER FROM DRAIN {:?}", buffer);
            // trace!("IS READABLE {:?}", self.stream.readable().await);

            match self.stream.try_read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    trace!("BYTES READ: {:?}", bytes_read);
                    if bytes_read == 0 {
                        break; // Stream is closed
                    }
                    self.buffer.extend_from_slice(&buffer[..bytes_read]);
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, yield to other tasks
                    trace!("YIELD");
                    tokio::task::yield_now();
                    // continue;
                }
                Err(err) => return Err(err), // Forward other errors
            }
        }

        Ok(())
    }
}

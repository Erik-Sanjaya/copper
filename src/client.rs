use std::{io::Cursor, net::SocketAddr};

use tokio::io::AsyncReadExt;

use crate::{
    data_types::{DataType, VarInt},
    packet::ServerBound,
    ProtocolError, State,
};

pub struct Client {
    addr: SocketAddr,
    stream: tokio::net::TcpStream,
    state: State,
}

impl Client {
    pub fn new(stream: tokio::net::TcpStream, addr: SocketAddr) -> Result<Self, ProtocolError> {
        Ok(Self {
            addr,
            stream,
            state: State::Handshaking,
        })
    }

    pub async fn handle_connection(&mut self) {
        loop {
            match self.read_stream().await {
                Ok(packet) => match packet {
                    ServerBound::Handshake(req) => panic!(),
                    ServerBound::Status(req) => panic!(),
                    ServerBound::Login(req) => panic!(),
                    ServerBound::Play(req) => panic!(),
                },
                Err(e) => panic!(),
            }
        }
    }

    async fn parse_packet(&mut self) -> Result<ServerBound, ProtocolError> {
        let mut length_buffer = [0; 5];

        // honestly, this peeking thing is only done because of the legacy ping
        // server list that has no length at all.
        // i don't like this double read on the length and have it be unused, so
        // i'll probably just hard code it here later.
        self.stream.peek(&mut length_buffer[..]).await;
        let mut length_cursor = Cursor::new(length_buffer.as_slice());
        let length = VarInt::read_from(&mut length_cursor)?;
        let mut packet = vec![0; length.size() + length.0 as usize];

        self.stream.read_exact(&mut packet).await?;

        let mut cursor = Cursor::new(packet.as_slice());
        ServerBound::parse_packet(&mut cursor, &self.state)
    }

    // fn parse_packet(&self, packet: &[u8]) -> Result<ServerBound, ProtocolError> {
    //     let mut cursor = Cursor::new(packet);
    //     ServerBound::parse_packet(&mut cursor, &self.state)
    // }

    pub async fn read_stream(&mut self) {
        unimplemented!()
    }
}

use std::{
    io::{Cursor, Read},
    string::FromUtf8Error,
};

use byteorder::ReadBytesExt;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum VarIntError {
    #[error("Bytes exceeded the limit of VarInt")]
    Overflow,
    #[error("Bytes too short")]
    MissingBytes,
}

#[derive(Debug)]
pub struct VarInt(pub i32);

impl VarInt {
    pub fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, VarIntError> {
        let mut result = 0;
        let mut shift = 0;

        loop {
            if shift >= 32 {
                return Err(VarIntError::Overflow);
            }

            let byte = match cursor.read_u8() {
                Ok(b) => b,
                Err(e) => {
                    error!("{:?}", e);
                    return Err(VarIntError::MissingBytes);
                }
            };

            result |= ((byte & 0x7F) as i32) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                break;
            }
        }

        Ok(Self(result))
    }

    pub fn write(&self, buffer: &mut Vec<u8>) {
        let mut value = self.0;

        loop {
            let mut temp = (value & 0x7f) as u8;
            value >>= 7;

            if value != 0 {
                temp |= 0x80;
                buffer.push(temp);
            } else {
                buffer.push(temp);
                break;
            }
        }
    }

    pub fn size(&self) -> usize {
        let mut value = self.0;
        let mut size = 0;

        loop {
            value >>= 7;
            size += 1;
            if value == 0 {
                break;
            }
        }

        size
    }
}

pub struct ProtocolString {
    pub length: VarInt,
    pub string: String,
}

#[derive(Debug, Error)]
pub enum ProtocolStringError {
    #[error("Packet length is invalid: {0}")]
    Length(#[source] VarIntError),
    #[error("Error with packet id: {0}")]
    PacketId(#[source] VarIntError),
    #[error("I/O error")]
    IOError(#[source] std::io::Error),
    #[error("Error parsing string from Utf8")]
    FromUtf8(#[source] FromUtf8Error),
}

impl ProtocolString {
    pub fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, ProtocolStringError> {
        let length = VarInt::read(cursor).map_err(ProtocolStringError::Length)?;
        let mut vec = vec![0; length.0 as usize];
        cursor
            .read_exact(&mut vec[..])
            .map_err(ProtocolStringError::IOError)?;
        let string = String::from_utf8(vec).map_err(ProtocolStringError::FromUtf8)?;

        Ok(Self { length, string })
    }

    pub fn write(&self, buffer: &mut Vec<u8>) {
        self.length.write(buffer);
        buffer.extend_from_slice(self.string.as_bytes())
    }
}

impl From<String> for ProtocolString {
    fn from(value: String) -> Self {
        let length = VarInt(value.len() as i32);
        Self {
            length,
            string: value,
        }
    }
}

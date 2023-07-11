use std::io::{Cursor, Read};

use byteorder::ReadBytesExt;

use tracing::error;

use crate::ProtocolError;

#[derive(Debug)]
pub struct VarInt(pub i32);

impl VarInt {
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let mut result = 0;
        let mut shift = 0;

        loop {
            if shift >= 32 {
                return Err(ProtocolError::Malformed);
            }

            let byte = match reader.read_u8() {
                Ok(b) => b,
                Err(e) => {
                    error!("{:?}", e);
                    return Err(ProtocolError::Missing);
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

    pub fn write_to(&self, buffer: &mut Vec<u8>) {
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

#[derive(Debug)]
pub struct ProtocolString {
    pub length: VarInt,
    pub string: String,
}

impl ProtocolString {
    pub fn read_from<R>(reader: &mut R) -> Result<Self, ProtocolError>
    where
        R: Read,
    {
        let length = VarInt::read_from(reader)?;
        let mut vec = vec![0; length.0 as usize];
        reader.read_exact(&mut vec[..])?;
        let string = String::from_utf8(vec).map_err(|_| ProtocolError::Malformed)?;

        Ok(Self { length, string })
    }

    pub fn write_to(&self, buffer: &mut Vec<u8>) {
        self.length.write_to(buffer);
        buffer.extend_from_slice(self.string.as_bytes())
    }

    pub fn size(&self) -> usize {
        self.length.size() + self.string.len()
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

use std::io::{Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt};

use tracing::error;

use crate::ProtocolError;

pub trait DataType: Sized {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError>;
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<usize, ProtocolError>;
    fn size(&self) -> usize;
}

#[derive(Debug)]
pub struct VarInt(pub i32);

impl DataType for VarInt {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let mut result = 0;
        let mut shift = 0;

        loop {
            if shift >= 32 {
                return Err(ProtocolError::Malformed);
            }

            let byte = match reader.read_u8() {
                Ok(b) => b,
                Err(e) => {
                    let mut buf = vec![];
                    reader.read_to_end(&mut buf)?;
                    error!(
                        "{:?} \n  | result: {} | shift: {} | reader: {:?}",
                        e, result, shift, buf
                    );
                    return Err(ProtocolError::Missing);
                }
            };

            result |= (i32::from(byte & 0x7F)) << shift;
            shift += 7;

            // trace!("RESULT INT {:?}", result);
            if byte & 0x80 == 0 {
                break;
            }
        }

        Ok(Self(result))
    }

    fn write_to<W: Write>(&self, buffer: &mut W) -> Result<usize, ProtocolError> {
        let mut value = self.0;
        let mut bytes = 0;

        loop {
            let mut temp = u8::try_from(value & 0x7f)?;
            value >>= 7;
            bytes += 1;

            if value != 0 {
                temp |= 0x80;
                buffer.write_u8(temp)?;
            } else {
                buffer.write_u8(temp)?;
                break;
            }
        }

        Ok(bytes)
    }

    fn size(&self) -> usize {
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

impl TryFrom<usize> for VarInt {
    type Error = ProtocolError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(i32::try_from(value)?))
    }
}

#[derive(Debug)]
pub struct ProtocolString {
    pub length: VarInt,
    pub string: String,
}

impl DataType for ProtocolString {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let length = VarInt::read_from(reader)?;
        let vec_len = usize::try_from(length.0)?;

        let mut vec = vec![0; vec_len];
        reader.read_exact(&mut vec[..])?;
        let string = String::from_utf8(vec).map_err(|_| ProtocolError::Malformed)?;

        Ok(Self { length, string })
    }

    fn write_to<W: Write>(&self, buffer: &mut W) -> Result<usize, ProtocolError> {
        self.length.write_to(buffer)?;
        buffer.write_all(self.string.as_bytes())?;

        Ok(self.size())
    }

    fn size(&self) -> usize {
        self.length.size() + self.string.len()
    }
}

impl TryFrom<String> for ProtocolString {
    type Error = ProtocolError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let length = VarInt(i32::try_from(value.len())?);

        Ok(Self {
            length,
            string: value,
        })
    }
}

impl TryFrom<&str> for ProtocolString {
    type Error = ProtocolError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let length = VarInt(i32::try_from(value.len())?);

        Ok(Self {
            length,
            string: value.into(),
        })
    }
}

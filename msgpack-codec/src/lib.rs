use bytes::BytesMut;
use rmpv::{self, Value};
use std::io::{self, Cursor};
use tokio_util::codec;

#[derive(Debug, Clone, Copy)]
pub struct Codec {
    pub max_msg_size: usize,
    pub max_depth: usize,
}

impl Codec {
    pub fn new() -> Self {
        Codec {
            max_msg_size: 1024,
            max_depth: 8,
        }
    }
}

impl codec::Decoder for Codec {
    type Item = Value;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() > self.max_msg_size {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "Message exceeded maximum size",
            ));
        }
        if src.len() == 0 {
            return Ok(None);
        }

        let mut cursor = Cursor::new(&src);
        match rmpv::decode::read_value_with_max_depth(&mut cursor, self.max_depth) {
            Ok(v) => {
                _ = src.split_to(cursor.position() as usize);
                Ok(Some(v))
            }
            Err(err) => match err {
                rmpv::decode::Error::InvalidMarkerRead(err) => match err.kind() {
                    std::io::ErrorKind::UnexpectedEof => Ok(None),
                    _ => Err(err),
                },
                rmpv::decode::Error::InvalidDataRead(err) => match err.kind() {
                    std::io::ErrorKind::UnexpectedEof => Ok(None),
                    _ => Err(err),
                },
                rmpv::decode::Error::DepthLimitExceeded => Err(io::Error::new(
                    io::ErrorKind::QuotaExceeded,
                    "Structure depth exceeded 8",
                )),
            },
        }
    }
}

impl codec::Encoder<Value> for Codec {
    type Error = io::Error;

    fn encode(&mut self, msg: Value, buf: &mut BytesMut) -> io::Result<()> {
        let mut data: Vec<u8> = Vec::new();
        rmpv::encode::write_value(&mut data, &msg)?;
        buf.extend(data);
        Ok(())
    }
}

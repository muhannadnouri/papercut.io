//! Fixed, bounded v4 control frames. Raw archive bytes are not control frames.

use std::io::{Read, Write};

use super::super::transport::ReceiverMessage;
use super::{Error, Result};
use crate::library_transfer::package::MAX_PACKAGE_BYTES;

pub(super) const STATUS_LIMIT: usize = 65_536;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub size: u64,
    pub digest: [u8; 32],
}

impl Offer {
    pub fn validate(self) -> Result<Self> {
        if self.size == 0 || self.size > MAX_PACKAGE_BYTES {
            return Err(Error::Protocol("Invalid archive size"));
        }
        Ok(self)
    }
}

pub(super) enum Frame {
    Offer(Offer),
    Ready(u64),
    Status(ReceiverMessage),
}

#[derive(Clone, Copy)]
pub(super) enum Expected {
    Offer,
    Ready { size: u64 },
    Result,
}

// Kept separate so the authenticated stream can start its assembly deadline
// after the first header byte, before reading the remaining bounded fields.
pub(super) fn read_body(reader: &mut impl Read, kind: u8, expected: Expected) -> Result<Frame> {
    let mut length = [0; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    match (expected, kind, length) {
        (Expected::Offer, 0x10, 40) => {
            let mut size = [0; 8];
            let mut digest = [0; 32];
            reader.read_exact(&mut size)?;
            reader.read_exact(&mut digest)?;
            Ok(Frame::Offer(
                Offer {
                    size: u64::from_be_bytes(size),
                    digest,
                }
                .validate()?,
            ))
        }
        (Expected::Ready { size }, 0x11, 8) => {
            let mut offset = [0; 8];
            reader.read_exact(&mut offset)?;
            let offset = u64::from_be_bytes(offset);
            if offset > size {
                return Err(Error::Protocol("Invalid resume offset"));
            }
            Ok(Frame::Ready(offset))
        }
        (Expected::Ready { .. } | Expected::Result, 0x12, 1..=STATUS_LIMIT) => {
            let mut body = vec![0; length];
            reader.read_exact(&mut body)?;
            let status = serde_json::from_slice(&body)
                .map_err(|_| Error::Protocol("Invalid receiver status"))?;
            if matches!(expected, Expected::Ready { .. })
                && !matches!(status, ReceiverMessage::Failed(_))
            {
                return Err(Error::Protocol("Only rejection is allowed before Ready"));
            }
            Ok(Frame::Status(status))
        }
        _ => Err(Error::Protocol("Unexpected control type or length")),
    }
}

pub(super) fn write_frame(writer: &mut impl Write, frame: &Frame) -> Result<()> {
    match frame {
        Frame::Offer(offer) => {
            offer.validate()?;
            writer.write_all(&[0x10, 0, 0, 0, 40])?;
            writer.write_all(&offer.size.to_be_bytes())?;
            writer.write_all(&offer.digest)?;
        }
        Frame::Ready(offset) => {
            writer.write_all(&[0x11, 0, 0, 0, 8])?;
            writer.write_all(&offset.to_be_bytes())?;
        }
        Frame::Status(status) => {
            let body =
                serde_json::to_vec(status).map_err(|_| Error::Protocol("Invalid local status"))?;
            if body.is_empty() || body.len() > STATUS_LIMIT {
                return Err(Error::Protocol("Receiver status exceeds limit"));
            }
            writer.write_all(&[0x12])?;
            writer.write_all(&(body.len() as u32).to_be_bytes())?;
            writer.write_all(&body)?;
        }
    }
    writer.flush()?;
    Ok(())
}

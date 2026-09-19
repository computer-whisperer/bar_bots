use std::io::{self, ErrorKind, Read, Write};

use serde::{Serialize, de::DeserializeOwned};

const MAX_FRAME: usize = 64 << 20;

/// Writes one message as a little-endian `u32` length followed by its postcard encoding.
pub fn write_frame<T: Serialize>(writer: &mut impl Write, message: &T) -> io::Result<()> {
    let body = postcard::to_stdvec(message).map_err(io::Error::other)?;
    let len = u32::try_from(body.len()).map_err(io::Error::other)?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// Reassembles frames from a stream that may deliver them in pieces, so it works on both
/// blocking and non-blocking sockets.
#[derive(Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    /// Returns `Ok(None)` when no complete frame is available yet (non-blocking source), and an
    /// `UnexpectedEof` error when the peer closed the connection.
    pub fn read<T: DeserializeOwned>(&mut self, source: &mut impl Read) -> io::Result<Option<T>> {
        loop {
            if let Some(message) = self.take_frame()? {
                return Ok(Some(message));
            }
            let mut chunk = [0u8; 64 * 1024];
            match source.read(&mut chunk) {
                Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(None),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    fn take_frame<T: DeserializeOwned>(&mut self) -> io::Result<Option<T>> {
        let Some(header) = self.buf.first_chunk::<4>() else {
            return Ok(None);
        };
        let len = u32::from_le_bytes(*header) as usize;
        if len > MAX_FRAME {
            return Err(io::Error::new(ErrorKind::InvalidData, format!("frame of {len} bytes")));
        }
        if self.buf.len() < 4 + len {
            return Ok(None);
        }
        let message = postcard::from_bytes(&self.buf[4..4 + len]).map_err(io::Error::other);
        self.buf.drain(..4 + len);
        message.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Commands, UnitId};

    /// Yields one byte per read, then `WouldBlock` once drained.
    struct Trickle(Vec<u8>);

    impl Read for Trickle {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.0.is_empty() {
                return Err(ErrorKind::WouldBlock.into());
            }
            out[0] = self.0.remove(0);
            Ok(1)
        }
    }

    #[test]
    fn reassembles_split_frames_and_reports_would_block() {
        let mut wire = Vec::new();
        write_frame(&mut wire, &Commands(vec![Command::Stop { unit: UnitId(7) }])).unwrap();
        write_frame(&mut wire, &Commands(vec![])).unwrap();
        let partial = wire.split_off(wire.len() - 1);

        let mut reader = FrameReader::default();
        let mut source = Trickle(wire);
        let first: Commands = reader.read(&mut source).unwrap().unwrap();
        assert!(matches!(first.0[..], [Command::Stop { unit: UnitId(7) }]));
        assert!(reader.read::<Commands>(&mut source).unwrap().is_none());

        source.0 = partial;
        let second: Commands = reader.read(&mut source).unwrap().unwrap();
        assert!(second.0.is_empty());
    }
}

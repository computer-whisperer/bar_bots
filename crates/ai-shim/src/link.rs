//! Connection to the bot process. Never blocks the engine thread for long: reads are
//! non-blocking and writes carry a short timeout.

use std::io;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use bot_protocol::{Commands, FrameReader, ToBot, socket_path, write_frame};

const WRITE_TIMEOUT: Duration = Duration::from_millis(250);

pub struct Link {
    stream: UnixStream,
    reader: FrameReader,
}

impl Link {
    pub fn connect() -> io::Result<Self> {
        let stream = UnixStream::connect(socket_path())?;
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        Ok(Link { stream, reader: FrameReader::default() })
    }

    pub fn send(&mut self, message: &ToBot) -> io::Result<()> {
        write_frame(&mut self.stream, message)
    }

    /// Returns the bot's reply if one has fully arrived.
    pub fn poll(&mut self) -> io::Result<Option<Commands>> {
        self.stream.set_nonblocking(true)?;
        let result = self.reader.read(&mut self.stream);
        self.stream.set_nonblocking(false)?;
        result
    }
}

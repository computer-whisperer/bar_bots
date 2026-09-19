//! The engine's autohost channel: the game server sends UDP event packets to us and accepts
//! plain-text server commands back (`rts/Net/AutohostInterface.cpp`).

use std::io::{self, ErrorKind};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

const SERVER_STARTPLAYING: u8 = 2;
const SERVER_GAMEOVER: u8 = 3;

pub enum Event {
    StartPlaying,
    GameOver { winning_ally_teams: Vec<u8> },
    Other,
}

pub struct Autohost {
    socket: UdpSocket,
    /// Learned from the first packet; commands cannot be sent before then.
    engine: Option<SocketAddr>,
}

impl Autohost {
    pub fn bind(port: u16) -> io::Result<Self> {
        Ok(Autohost { socket: UdpSocket::bind(("127.0.0.1", port))?, engine: None })
    }

    pub fn receive(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        self.socket.set_read_timeout(Some(timeout))?;
        let mut packet = [0u8; 65536];
        let len = match self.socket.recv_from(&mut packet) {
            Ok((len, from)) => {
                self.engine = Some(from);
                len
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return Ok(None),
            Err(e) => return Err(e),
        };
        Ok(Some(match packet[..len] {
            [SERVER_STARTPLAYING, ..] => Event::StartPlaying,
            // [id, message size, reporting player, winning ally teams...]
            [SERVER_GAMEOVER, _, _, ref winners @ ..] => Event::GameOver { winning_ally_teams: winners.to_vec() },
            _ => Event::Other,
        }))
    }

    pub fn send(&self, command: &str) -> io::Result<()> {
        let Some(engine) = self.engine else {
            return Err(io::Error::other("no packet received from the engine yet"));
        };
        self.socket.send_to(command.as_bytes(), engine).map(drop)
    }
}

//! Minimal length-prefixed TCP framing for the toy's transport worker. One frame
//! = one fact's canonical bytes; the sender connects, writes its frames, and
//! closes, and the reader drains to EOF.
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;

/// 4-byte big-endian length prefix + payload.
pub fn write_frame(s: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    s.write_all(&(bytes.len() as u32).to_be_bytes())?;
    s.write_all(bytes)
}

/// Read one frame; `Ok(None)` at a clean EOF (no partial frame pending).
pub fn read_frame(s: &mut impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    if let Err(e) = s.read_exact(&mut len) {
        if e.kind() == ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e);
    }
    let n = u32::from_be_bytes(len) as usize;
    let mut buf = vec![0u8; n];
    s.read_exact(&mut buf)?;
    Ok(Some(buf))
}

/// Connect to `addr`, write all frames, and close.
pub fn send_frames(addr: &str, frames: &[Vec<u8>]) -> std::io::Result<()> {
    let mut s = TcpStream::connect(addr)?;
    for f in frames {
        write_frame(&mut s, f)?;
    }
    s.flush()
}

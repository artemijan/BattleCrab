//! L2 packet framing: a 2-byte little-endian length header whose value
//! includes the header itself (`ConnectionConfig.HEADER_SIZE = 2`).

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const HEADER_SIZE: usize = 2;

/// Reads one frame; returns the payload (without header), or `None` on a
/// clean EOF before the header. Oversized/undersized frames are IO errors.
pub async fn read_frame<R: AsyncRead + Unpin>(
    read: &mut R,
    max_payload: usize,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; HEADER_SIZE];
    match read.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let total = u16::from_le_bytes(header) as usize;
    if total < HEADER_SIZE || total - HEADER_SIZE > max_payload {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("bad frame length {total}"),
        ));
    }
    let mut payload = vec![0u8; total - HEADER_SIZE];
    read.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

/// Appends one framed packet — header (payload length + 2, LE) then the
/// payload — to `buf`, without touching the socket.
///
/// The building block for coalescing: sockets here run with `TCP_NODELAY`, so
/// a header written separately from its body leaves the wire as its own 2-byte
/// segment. Callers with several packets to send should frame them all into
/// one buffer and issue a single `write_all` (see the game server's
/// per-connection task), which collapses both the syscall count and the
/// segment count to one per wakeup instead of two per packet.
pub fn frame_into(buf: &mut Vec<u8>, payload: &[u8]) {
    let total = (payload.len() + HEADER_SIZE) as u16;
    buf.extend_from_slice(&total.to_le_bytes());
    buf.extend_from_slice(payload);
}

/// Writes one frame: header (payload length + 2, LE) followed by the payload.
///
/// Header and payload go out in a **single** `write_all`; splitting them was
/// worth a wasted syscall and a 2-byte TCP segment per packet under
/// `TCP_NODELAY`. Use [`frame_into`] directly when more than one packet is
/// ready, so they share a write too.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    write: &mut W,
    payload: &[u8],
) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(payload.len() + HEADER_SIZE);
    frame_into(&mut buf, payload);
    write.write_all(&buf).await?;
    write.flush().await
}

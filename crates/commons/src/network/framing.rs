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

/// Writes one frame: header (payload length + 2, LE) followed by the payload.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    write: &mut W,
    payload: &[u8],
) -> std::io::Result<()> {
    let total = (payload.len() + HEADER_SIZE) as u16;
    write.write_all(&total.to_le_bytes()).await?;
    write.write_all(payload).await?;
    write.flush().await
}

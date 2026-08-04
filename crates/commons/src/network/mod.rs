//! Reusable network core (replaces Async-mmocore): L2 framing and packet
//! buffer helpers. Shared by the login server and, later, the game server.

mod framing;
mod packet;

pub use framing::{HEADER_SIZE, frame_into, read_frame, write_frame};
pub use packet::{PacketReader, PacketWriter};

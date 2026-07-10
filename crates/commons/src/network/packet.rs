//! Packet body read/write helpers with L2 conventions: little-endian
//! integers, `f64` doubles, UTF-16LE null-terminated strings.
//! Port of the `ReadableBuffer`/`WritableBuffer` surface actually used.

pub struct PacketReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> PacketReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.remaining() < n {
            return None;
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(slice)
    }

    pub fn read_u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }

    pub fn read_i16(&mut self) -> Option<i16> {
        self.take(2).map(|s| i16::from_le_bytes(s.try_into().unwrap()))
    }

    pub fn read_i32(&mut self) -> Option<i32> {
        self.take(4).map(|s| i32::from_le_bytes(s.try_into().unwrap()))
    }

    pub fn read_i64(&mut self) -> Option<i64> {
        self.take(8).map(|s| i64::from_le_bytes(s.try_into().unwrap()))
    }

    pub fn read_f64(&mut self) -> Option<f64> {
        self.take(8).map(|s| f64::from_le_bytes(s.try_into().unwrap()))
    }

    pub fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        self.take(n)
    }

    /// Null-terminated UTF-16LE string (`readString`).
    pub fn read_string(&mut self) -> Option<String> {
        let mut units = Vec::new();
        loop {
            let unit = u16::from_le_bytes(self.take(2)?.try_into().unwrap());
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        Some(String::from_utf16_lossy(&units))
    }
}

#[derive(Default)]
pub struct PacketWriter {
    data: Vec<u8>,
}

impl PacketWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_u8(&mut self, v: u8) {
        self.data.push(v);
    }

    pub fn write_i16(&mut self, v: i16) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_i32(&mut self, v: i32) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_i64(&mut self, v: i64) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_f64(&mut self, v: f64) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_bytes(&mut self, v: &[u8]) {
        self.data.extend_from_slice(v);
    }

    /// Null-terminated UTF-16LE string (`writeString`).
    pub fn write_string(&mut self, s: &str) {
        for unit in s.encode_utf16() {
            self.data.extend_from_slice(&unit.to_le_bytes());
        }
        self.data.extend_from_slice(&0u16.to_le_bytes());
    }

    /// `writeSizedString`: a 2-byte char-count prefix + UTF-16LE, no null
    /// terminator (used in `UserInfo` blocks).
    pub fn write_sized_string(&mut self, s: &str) {
        let units: Vec<u16> = s.encode_utf16().collect();
        self.write_i16(units.len() as i16);
        for unit in units {
            self.data.extend_from_slice(&unit.to_le_bytes());
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_string_roundtrip() {
        let mut w = PacketWriter::new();
        w.write_string("Bartz");
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 12); // 5 chars + terminator, 2 bytes each
        let mut r = PacketReader::new(&bytes);
        assert_eq!(r.read_string().unwrap(), "Bartz");
    }
}

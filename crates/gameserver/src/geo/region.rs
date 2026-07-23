//! One loaded `x_y.l2j` geodata region — port of `geodata/regions/Region` and
//! the three `geodata/blocks/*` classes.
//!
//! Unlike Java (which parses every block into heap objects, hence its
//! multi-GB geodata heap), the region keeps the raw little-endian file bytes
//! memory-mapped and answers queries straight from them; the only parsed
//! state is a 64K-entry byte-offset index locating each block (blocks are
//! variable-length because of multilayer cells). See PLAN_GAME_SERVER §risks:
//! "mmap + read-only shared geodata". Runtime NSWE editing (`setNearestNswe`,
//! doors/fences punching holes) is not ported yet — the data is read-only.

use std::fs::File;
use std::io;
use std::path::Path;

/// `IBlock`/`IRegion` geometry constants.
pub const BLOCK_CELLS_X: i32 = 8;
pub const BLOCK_CELLS_Y: i32 = 8;
pub const BLOCK_CELLS: i32 = BLOCK_CELLS_X * BLOCK_CELLS_Y;
pub const REGION_BLOCKS_X: i32 = 256;
pub const REGION_BLOCKS_Y: i32 = 256;
pub const REGION_BLOCKS: i32 = REGION_BLOCKS_X * REGION_BLOCKS_Y;
pub const REGION_CELLS_X: i32 = REGION_BLOCKS_X * BLOCK_CELLS_X;
pub const REGION_CELLS_Y: i32 = REGION_BLOCKS_Y * BLOCK_CELLS_Y;

const TYPE_FLAT: u8 = 0;
const TYPE_COMPLEX: u8 = 1;
const TYPE_MULTILAYER: u8 = 2;

/// Region bytes: mmap'd from disk in production, owned for synthetic regions
/// in tests / the future geo editor.
enum Bytes {
    Mapped(memmap2::Mmap),
    Owned(Vec<u8>),
}

impl Bytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            Bytes::Mapped(m) => m,
            Bytes::Owned(v) => v,
        }
    }
}

pub struct Region {
    bytes: Bytes,
    /// Byte offset of each block's type byte, indexed like Java's `_blocks`
    /// array: `((geoX/8) % 256) * 256 + (geoY/8) % 256`.
    block_offsets: Box<[u32]>,
}

impl Region {
    pub fn load(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        // SAFETY: the geodata files are opened read-only and never truncated
        // or rewritten while the server runs (read-only reference data).
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Self::from_bytes(Bytes::Mapped(mmap))
    }

    /// Build a region from an in-memory `.l2j` image (tests, editor).
    pub fn from_owned(data: Vec<u8>) -> io::Result<Self> {
        Self::from_bytes(Bytes::Owned(data))
    }

    /// Single validation/index pass over all 65536 blocks — the equivalent of
    /// Java's eager `new FlatBlock/ComplexBlock/MultilayerBlock` parse loop.
    fn from_bytes(bytes: Bytes) -> io::Result<Self> {
        let data = bytes.as_slice();
        let mut offsets = vec![0u32; REGION_BLOCKS as usize];
        let mut pos: usize = 0;
        let bad = |msg: &str| io::Error::new(io::ErrorKind::InvalidData, msg.to_string());
        for offset in offsets.iter_mut() {
            *offset = pos as u32;
            let &block_type = data.get(pos).ok_or_else(|| bad("truncated region file"))?;
            pos += 1;
            match block_type {
                TYPE_FLAT => pos += 2,
                TYPE_COMPLEX => pos += BLOCK_CELLS as usize * 2,
                TYPE_MULTILAYER => {
                    for _ in 0..BLOCK_CELLS {
                        let &layers = data.get(pos).ok_or_else(|| bad("truncated region file"))?;
                        // Java: `(nLayers <= 0) || (nLayers > 125)` → corrupt.
                        if layers == 0 || layers > 125 {
                            return Err(bad("invalid layer count in multilayer block"));
                        }
                        pos += 1 + layers as usize * 2;
                    }
                }
                _ => return Err(bad("invalid block type")),
            }
            if pos > data.len() {
                return Err(bad("truncated region file"));
            }
        }
        Ok(Self {
            bytes,
            block_offsets: offsets.into_boxed_slice(),
        })
    }

    fn i16_at(&self, pos: usize) -> i16 {
        let d = self.bytes.as_slice();
        i16::from_le_bytes([d[pos], d[pos + 1]])
    }

    /// Java `Region.getBlock` index → this region's byte offset of the block.
    fn block_offset(&self, geo_x: i32, geo_y: i32) -> usize {
        let idx = (((geo_x / BLOCK_CELLS_X) % REGION_BLOCKS_X) * REGION_BLOCKS_Y)
            + ((geo_y / BLOCK_CELLS_Y) % REGION_BLOCKS_Y);
        self.block_offsets[idx as usize] as usize
    }

    /// Cell index inside a block (Java: `(geoX % 8) * 8 + geoY % 8`).
    fn cell_index(geo_x: i32, geo_y: i32) -> usize {
        ((geo_x % BLOCK_CELLS_X) * BLOCK_CELLS_Y + (geo_y % BLOCK_CELLS_Y)) as usize
    }

    /// Byte offset of a multilayer cell's layer-count byte, walking the
    /// preceding variable-length cells (Java `getCellDataOffset`).
    fn multilayer_cell_offset(&self, block_data_start: usize, geo_x: i32, geo_y: i32) -> usize {
        let data = self.bytes.as_slice();
        let mut pos = block_data_start;
        for _ in 0..Self::cell_index(geo_x, geo_y) {
            pos += 1 + data[pos] as usize * 2;
        }
        pos
    }

    /// Java `checkNearestNswe` — can the cell nearest to `world_z` be exited
    /// in every direction of `nswe`? Flat blocks are open in all directions.
    pub fn check_nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32, nswe: u8) -> bool {
        let pos = self.block_offset(geo_x, geo_y);
        match self.bytes.as_slice()[pos] {
            TYPE_FLAT => true,
            TYPE_COMPLEX => {
                let data = self.i16_at(pos + 1 + Self::cell_index(geo_x, geo_y) * 2);
                (cell_nswe(data) & nswe) == nswe
            }
            _ => {
                let cell = self.multilayer_cell_offset(pos + 1, geo_x, geo_y);
                (cell_nswe(self.nearest_layer(cell, world_z)) & nswe) == nswe
            }
        }
    }

    /// The raw NSWE bits of the cell nearest to `world_z` (flat blocks are open
    /// in every direction). Used by the admin geo-editor to read the current
    /// value before OR/AND-ing an edited bit.
    pub fn nearest_nswe(&self, geo_x: i32, geo_y: i32, world_z: i32) -> u8 {
        let pos = self.block_offset(geo_x, geo_y);
        match self.bytes.as_slice()[pos] {
            TYPE_FLAT => super::NSWE_ALL,
            TYPE_COMPLEX => cell_nswe(self.i16_at(pos + 1 + Self::cell_index(geo_x, geo_y) * 2)),
            _ => {
                let cell = self.multilayer_cell_offset(pos + 1, geo_x, geo_y);
                cell_nswe(self.nearest_layer(cell, world_z))
            }
        }
    }

    /// Java `getNearestZ` — the height of the cell/layer closest to `world_z`.
    pub fn nearest_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        let pos = self.block_offset(geo_x, geo_y);
        match self.bytes.as_slice()[pos] {
            TYPE_FLAT => self.i16_at(pos + 1) as i32,
            TYPE_COMPLEX => cell_height(self.i16_at(pos + 1 + Self::cell_index(geo_x, geo_y) * 2)),
            _ => {
                let cell = self.multilayer_cell_offset(pos + 1, geo_x, geo_y);
                cell_height(self.nearest_layer(cell, world_z))
            }
        }
    }

    /// Java `getNextLowerZ` — highest layer at or below `world_z` (or
    /// `world_z` itself if none).
    pub fn next_lower_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        let pos = self.block_offset(geo_x, geo_y);
        match self.bytes.as_slice()[pos] {
            TYPE_FLAT => {
                let height = self.i16_at(pos + 1) as i32;
                if height <= world_z {
                    height
                } else {
                    world_z
                }
            }
            TYPE_COMPLEX => {
                cell_height(self.i16_at(pos + 1 + Self::cell_index(geo_x, geo_y) * 2)).min(world_z)
            }
            _ => {
                let cell = self.multilayer_cell_offset(pos + 1, geo_x, geo_y);
                let mut lower = i32::MIN;
                for layer in self.layers(cell) {
                    let layer_z = cell_height(layer);
                    if layer_z == world_z {
                        return layer_z;
                    }
                    if layer_z < world_z && layer_z > lower {
                        lower = layer_z;
                    }
                }
                if lower == i32::MIN {
                    world_z
                } else {
                    lower
                }
            }
        }
    }

    /// Java `getNextHigherZ` — lowest layer at or above `world_z` (or
    /// `world_z` itself if none).
    pub fn next_higher_z(&self, geo_x: i32, geo_y: i32, world_z: i32) -> i32 {
        let pos = self.block_offset(geo_x, geo_y);
        match self.bytes.as_slice()[pos] {
            TYPE_FLAT => {
                let height = self.i16_at(pos + 1) as i32;
                if height >= world_z {
                    height
                } else {
                    world_z
                }
            }
            TYPE_COMPLEX => {
                cell_height(self.i16_at(pos + 1 + Self::cell_index(geo_x, geo_y) * 2)).max(world_z)
            }
            _ => {
                let cell = self.multilayer_cell_offset(pos + 1, geo_x, geo_y);
                let mut higher = i32::MAX;
                for layer in self.layers(cell) {
                    let layer_z = cell_height(layer);
                    if layer_z == world_z {
                        return layer_z;
                    }
                    if layer_z > world_z && layer_z < higher {
                        higher = layer_z;
                    }
                }
                if higher == i32::MAX {
                    world_z
                } else {
                    higher
                }
            }
        }
    }

    /// Java `MultilayerBlock.getNearestLayer`: exact z match wins, otherwise
    /// smallest |dz|; the first layer seeds the search (so ties keep the
    /// earlier layer).
    fn nearest_layer(&self, cell_offset: usize, world_z: i32) -> i16 {
        let mut nearest_dz = 0;
        let mut nearest = 0i16;
        for (i, layer) in self.layers(cell_offset).enumerate() {
            let layer_z = cell_height(layer);
            if layer_z == world_z {
                return layer;
            }
            let dz = (layer_z - world_z).abs();
            if i == 0 || dz < nearest_dz {
                nearest_dz = dz;
                nearest = layer;
            }
        }
        nearest
    }

    fn layers(&self, cell_offset: usize) -> impl Iterator<Item = i16> + '_ {
        let count = self.bytes.as_slice()[cell_offset] as usize;
        (0..count).map(move |i| self.i16_at(cell_offset + 1 + i * 2))
    }
}

/// Low 4 bits of a complex/multilayer cell short: NSWE mask.
fn cell_nswe(data: i16) -> u8 {
    (data & 0x000F) as u8
}

/// Bits 4..15, arithmetic-shifted: height in world units (Java
/// `(short) (data & 0x0fff0) >> 1`).
fn cell_height(data: i16) -> i32 {
    ((data & !0x000F) >> 1) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::NSWE_ALL;

    /// Encode a cell short the way the files store it: height doubled into
    /// bits 4..15, NSWE in the low nibble.
    pub(crate) fn encode_cell(height: i16, nswe: u8) -> i16 {
        (((height as i32) << 1) as i16 & !0x000F) | nswe as i16
    }

    /// The stored short keeps NSWE in the low nibble and `height << 1` above
    /// it (`(short) (data & 0x0fff0) >> 1` to decode), so representable
    /// heights are multiples of 8 — which is what the client grid uses.
    #[test]
    fn cell_encoding_round_trips() {
        for h in [-16384, -3480, -8, 0, 8, 1000, 16376] {
            let cell = encode_cell(h, 0b1010);
            assert_eq!(cell_height(cell), h as i32, "height {h}");
            assert_eq!(cell_nswe(cell), 0b1010);
        }
    }

    #[test]
    fn flat_complex_and_multilayer_blocks_answer_queries() {
        // Region image: block 0 flat at z=-100, block 1 complex (all cells
        // z=48, NSWE_ALL), block 2 multilayer (cell 0 has layers z=0 and
        // z=1000, rest single-layer z=0), remaining blocks flat z=0.
        let mut img = Vec::new();
        img.push(0u8); // flat
        img.extend_from_slice(&(-100i16).to_le_bytes());
        img.push(1u8); // complex
        for _ in 0..BLOCK_CELLS {
            img.extend_from_slice(&encode_cell(48, NSWE_ALL).to_le_bytes());
        }
        img.push(2u8); // multilayer
        img.push(2u8); // cell 0: two layers
        img.extend_from_slice(&encode_cell(0, NSWE_ALL).to_le_bytes());
        img.extend_from_slice(&encode_cell(1000, NSWE_ALL).to_le_bytes());
        for _ in 1..BLOCK_CELLS {
            img.push(1u8);
            img.extend_from_slice(&encode_cell(0, NSWE_ALL).to_le_bytes());
        }
        for _ in 3..REGION_BLOCKS {
            img.push(0u8);
            img.extend_from_slice(&0i16.to_le_bytes());
        }
        let region = Region::from_owned(img).unwrap();

        // Block 0 = geo cells (0..8, 0..8): flat.
        assert_eq!(region.nearest_z(0, 0, 12345), -100);
        assert!(region.check_nearest_nswe(0, 0, 0, NSWE_ALL));
        assert_eq!(
            region.next_lower_z(0, 0, -200),
            -200,
            "flat above worldZ → worldZ"
        );
        assert_eq!(region.next_higher_z(0, 0, -200), -100);

        // Block 1 = geo cells x 0..8, y 8..16 (blocks are laid out y-minor).
        assert_eq!(region.nearest_z(3, 9, 0), 48);
        assert_eq!(region.next_higher_z(3, 9, 60), 60);

        // Block 2 = geo cells x 0..8, y 16..24; cell (0,16) is dual-layer.
        assert_eq!(region.nearest_z(0, 16, 900), 1000, "picks the closer layer");
        assert_eq!(region.nearest_z(0, 16, 100), 0);
        assert_eq!(region.next_higher_z(0, 16, 10), 1000);
        assert_eq!(region.next_lower_z(0, 16, 900), 0);
        // Equidistant tie (worldZ=500): the first layer seeds the search and
        // later layers need a strictly smaller |dz| — Java keeps layer 0.
        assert_eq!(region.nearest_z(0, 16, 500), 0);
    }

    #[test]
    fn corrupt_region_is_rejected() {
        assert!(Region::from_owned(vec![]).is_err());
        assert!(Region::from_owned(vec![7]).is_err(), "unknown block type");
        // Multilayer with a zero layer count.
        assert!(Region::from_owned(vec![2, 0]).is_err());
    }
}

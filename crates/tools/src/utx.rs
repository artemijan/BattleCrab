//! Icon textures out of the client's Unreal packages.
//!
//! The `.utx` files under `systextures/` are UE2 packages behind the same
//! `Lineage2Ver` header as everything else, but XOR-ciphered even in clients
//! whose `.dat` files moved to RSA — and the key is derived from the file
//! *name*, not the version, so it is recovered here from the known package
//! magic instead of a table.
//!
//! Inside, each icon is its own 32x32 `Texture` export (there is no atlas):
//! `icon.weapon_arcana_mace_i00` in a grp file means package `Icon.utx`,
//! export `weapon_arcana_mace_i00`. The export's properties give the pixel
//! format and size, and its mipmaps follow — but this client's engine writes
//! extra material data (source path, shader text) between the two, with no
//! length prefix. Rather than reverse that, the mip is anchored on what *is*
//! verifiable: each mip ends `USize:u32 VSize:u32 UBits:u8 VBits:u8`, its
//! data length is fixed by format and size, and the `u32` lazy-array skip
//! offset before the data must equal the data's end — three independent
//! checks, so a match is not luck.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `0x9E2A83C1`, first bytes of every Unreal package, and the anchor the XOR
/// key is recovered from.
const PACKAGE_MAGIC: [u8; 4] = [0xC1, 0x83, 0x2A, 0x9E];

/// Object flag marking a state frame before the property list. Textures never
/// carry one; seeing it means this parser's simplification does not hold.
const RF_HAS_STACK: u32 = 0x0200_0000;

/// The `ETextureFormat` values this exporter can decode to RGBA.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Dxt1,
    Dxt3,
    Dxt5,
    Rgba8,
}

impl Format {
    fn from_id(id: u8) -> Option<Format> {
        Some(match id {
            3 => Format::Dxt1,
            5 => Format::Rgba8,
            7 => Format::Dxt3,
            8 => Format::Dxt5,
            _ => return None,
        })
    }

    /// Stored byte count of one mip level at `w` x `h`.
    fn data_len(self, w: u32, h: u32) -> usize {
        let blocks = (w.div_ceil(4) * h.div_ceil(4)) as usize;
        match self {
            Format::Dxt1 => blocks * 8,
            Format::Dxt3 | Format::Dxt5 => blocks * 16,
            Format::Rgba8 => (w * h * 4) as usize,
        }
    }
}

/// A decoded texture: straight RGBA rows, top-left first.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

// --- package reading ---------------------------------------------------------

struct Export {
    class: i32,
    name: String,
    flags: u32,
    size: usize,
    offset: usize,
}

/// One opened, deciphered `.utx` with its tables parsed.
pub struct Package {
    data: Vec<u8>,
    names: Vec<String>,
    /// Import object names, for resolving an export's class.
    imports: Vec<String>,
    exports: Vec<Export>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        let end = end.ok_or_else(|| format!("truncated at {:#x}+{n}", self.pos))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }

    /// Unreal's signed variable-length integer: sign and continuation in the
    /// first byte, seven data bits per byte after.
    fn compact(&mut self) -> Result<i32, String> {
        let b0 = self.u8()?;
        let negative = b0 & 0x80 != 0;
        let mut value = (b0 & 0x3F) as i64;
        if b0 & 0x40 != 0 {
            let mut shift = 6;
            loop {
                let b = self.u8()?;
                value |= ((b & 0x7F) as i64) << shift;
                shift += 7;
                if b & 0x80 == 0 || shift > 34 {
                    break;
                }
            }
        }
        Ok(if negative {
            -value as i32
        } else {
            value as i32
        })
    }

    /// Length-prefixed string; negative length means UTF-16.
    fn fstring(&mut self) -> Result<String, String> {
        let n = self.compact()?;
        if n >= 0 {
            let bytes = self.take(n as usize)?;
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            Ok(bytes[..end].iter().map(|&b| b as char).collect())
        } else {
            let units: Vec<u16> = self
                .take(-n as usize * 2)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect();
            let text = String::from_utf16(&units).map_err(|e| e.to_string())?;
            Ok(text.trim_end_matches('\0').to_owned())
        }
    }
}

/// Strip the `Lineage2Ver` header and undo the XOR. The key is whatever maps
/// the first ciphered byte onto the package magic — verified against the
/// remaining three magic bytes, so a non-XOR or corrupt file cannot slip
/// through as garbage.
fn decipher(raw: &[u8]) -> Result<Vec<u8>, String> {
    let version =
        crate::client_dat::read_version(raw).ok_or_else(|| "no Lineage2Ver header".to_string())?;
    let body = &raw[crate::client_dat::HEADER_LEN..];
    if body.len() < 4 {
        return Err("file is just a header".into());
    }
    let key = body[0] ^ PACKAGE_MAGIC[0];
    if body[1] ^ key != PACKAGE_MAGIC[1]
        || body[2] ^ key != PACKAGE_MAGIC[2]
        || body[3] ^ key != PACKAGE_MAGIC[3]
    {
        return Err(format!("Ver{version} body does not XOR onto a package"));
    }
    Ok(body.iter().map(|b| b ^ key).collect())
}

impl Package {
    pub fn load(path: &Path) -> Result<Package, String> {
        let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let data = decipher(&raw).map_err(|e| format!("{}: {e}", path.display()))?;

        let mut r = Reader {
            data: &data,
            pos: 4,
        };
        let _versions = r.u32()?; // engine and licensee, unused here
        let _flags = r.u32()?;
        let name_count = r.i32()?;
        let name_offset = r.i32()? as usize;
        let export_count = r.i32()?;
        let export_offset = r.i32()? as usize;
        let import_count = r.i32()?;
        let import_offset = r.i32()? as usize;

        let mut r = Reader {
            data: &data,
            pos: name_offset,
        };
        let mut names = Vec::with_capacity(name_count as usize);
        for _ in 0..name_count {
            names.push(r.fstring()?);
            r.u32()?; // name flags
        }
        let name = |index: i32| -> Result<&String, String> {
            names
                .get(index as usize)
                .ok_or_else(|| format!("name index {index} out of range"))
        };

        let mut r = Reader {
            data: &data,
            pos: import_offset,
        };
        let mut imports = Vec::with_capacity(import_count as usize);
        for _ in 0..import_count {
            r.compact()?; // class package
            r.compact()?; // class name
            r.i32()?; // enclosing package
            imports.push(name(r.compact()?)?.clone());
        }

        let mut r = Reader {
            data: &data,
            pos: export_offset,
        };
        let mut exports = Vec::with_capacity(export_count as usize);
        for _ in 0..export_count {
            let class = r.compact()?;
            r.compact()?; // super
            r.i32()?; // group
            let entry_name = name(r.compact()?)?.clone();
            let flags = r.u32()?;
            let size = r.compact()? as usize;
            let offset = if size > 0 { r.compact()? as usize } else { 0 };
            exports.push(Export {
                class,
                name: entry_name,
                flags,
                size,
                offset,
            });
        }

        Ok(Package {
            data,
            names,
            imports,
            exports,
        })
    }

    /// The export's class name: negative indices point into the import table,
    /// positive ones at other exports, zero at the base `Object`.
    fn class_name(&self, class: i32) -> &str {
        match class {
            0 => "Object",
            c if c < 0 => self
                .imports
                .get((-c - 1) as usize)
                .map(String::as_str)
                .unwrap_or("?"),
            c => self
                .exports
                .get((c - 1) as usize)
                .map(|e| e.name.as_str())
                .unwrap_or("?"),
        }
    }

    /// Names of every `Texture` export addressable by name, in file order. A
    /// few dozen names recur under different package groups; those collapse
    /// to their first occurrence here because that is the one [`Self::texture`]'s
    /// first-match lookup — like the client's — would resolve anyway.
    pub fn texture_names(&self) -> impl Iterator<Item = &str> {
        let mut seen = std::collections::BTreeSet::new();
        self.exports
            .iter()
            .filter(|e| self.class_name(e.class).eq_ignore_ascii_case("Texture"))
            .map(|e| e.name.as_str())
            .filter(move |n| seen.insert(n.to_lowercase()))
    }

    /// Decode the `Texture` export called `name` (case-insensitive).
    pub fn texture(&self, name: &str) -> Result<Rgba, String> {
        let export = self
            .exports
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("no export named {name}"))?;
        let class = self.class_name(export.class);
        if !class.eq_ignore_ascii_case("Texture") {
            return Err(format!("{name} is a {class}, not a Texture"));
        }
        if export.flags & RF_HAS_STACK != 0 {
            return Err(format!("{name} has a state frame"));
        }
        let object = self
            .data
            .get(export.offset..export.offset + export.size)
            .ok_or_else(|| format!("{name}: object bounds out of file"))?;
        self.decode_texture(name, export.offset, object)
    }

    fn decode_texture(&self, name: &str, base: usize, object: &[u8]) -> Result<Rgba, String> {
        let props = self
            .read_properties(object)
            .map_err(|e| format!("{name}: {e}"))?;
        let format_id = *props
            .get("Format")
            .ok_or_else(|| format!("{name}: no Format"))?;
        let format = Format::from_id(format_id as u8)
            .ok_or_else(|| format!("{name}: unsupported texture format {format_id}"))?;

        // One mip record is `skip:u32, count:compact, data, USize:u32,
        // VSize:u32, UBits:u8, VBits:u8`, and they are stored largest level
        // first. Some icons carry only the full-size level, others a whole
        // chain down to 1x1 — so walk record-by-record backwards from the
        // object's end, each step validated by its dimensions being powers of
        // two and its lazy-array skip offset landing exactly on its data's
        // end, and keep the last record the walk reaches: the full-size one.
        let mut end = object.len();
        let mut full: Option<(u32, u32, usize)> = None; // width, height, data start
        while end >= 10 {
            let tail = end - 10;
            let width = u32::from_le_bytes(object[tail..tail + 4].try_into().unwrap());
            let height = u32::from_le_bytes(object[tail + 4..tail + 8].try_into().unwrap());
            let (u_bits, v_bits) = (object[tail + 8], object[tail + 9]);
            if u_bits >= 32 || v_bits >= 32 || width != 1 << u_bits || height != 1 << v_bits {
                break;
            }
            let len = format.data_len(width, height);
            let Some(start) = tail.checked_sub(len) else {
                break;
            };
            let Some(skip_at) = start.checked_sub(compact_len(len as i64) + 4) else {
                break;
            };
            let skip =
                u32::from_le_bytes(object[skip_at..skip_at + 4].try_into().unwrap()) as usize;
            if skip != base + tail {
                break;
            }
            full = Some((width, height, start));
            end = skip_at;
        }
        let Some((width, height, start)) = full else {
            return Err(format!("{name}: no valid mip record at the object's end"));
        };

        let data = &object[start..start + format.data_len(width, height)];
        let pixels = match format {
            Format::Dxt1 => decode_dxt(data, width, height, DxtAlpha::Opaque),
            Format::Dxt3 => decode_dxt(data, width, height, DxtAlpha::Explicit),
            Format::Dxt5 => decode_dxt(data, width, height, DxtAlpha::Interpolated),
            Format::Rgba8 => bgra_to_rgba(data),
        };
        Ok(Rgba {
            width,
            height,
            pixels,
        })
    }

    /// Walk the object's property list up to its `None` terminator, keeping
    /// the plain numeric values. Only what a `Texture` needs is interpreted;
    /// everything else is skipped by its declared size.
    fn read_properties(&self, object: &[u8]) -> Result<BTreeMap<String, i64>, String> {
        let mut r = Reader {
            data: object,
            pos: 0,
        };
        let mut props = BTreeMap::new();
        loop {
            let name = self
                .names
                .get(r.compact()? as usize)
                .ok_or("property name out of range")?
                .clone();
            if name == "None" {
                return Ok(props);
            }
            let info = r.u8()?;
            let kind = info & 0x0F;
            if kind == 10 {
                r.compact()?; // struct type name
            }
            let size = match (info >> 4) & 0x7 {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 12,
                4 => 16,
                5 => r.u8()? as usize,
                6 => u16::from_le_bytes(r.take(2)?.try_into().unwrap()) as usize,
                _ => r.u32()? as usize,
            };
            // Booleans keep their value in the array bit and carry no payload.
            if kind == 3 {
                props.insert(name, (info >> 7) as i64);
                continue;
            }
            if info & 0x80 != 0 {
                // Array index: one byte below 128, longer forms above.
                let b0 = r.u8()?;
                if b0 & 0x80 != 0 {
                    if b0 & 0x40 != 0 {
                        r.take(3)?;
                    } else {
                        r.take(1)?;
                    }
                }
            }
            let value = r.take(size)?;
            let numeric = match (kind, size) {
                (1 | 2 | 5 | 6, 1) => value[0] as i64,
                (1 | 2 | 5 | 6, 4) => i32::from_le_bytes(value.try_into().unwrap()) as i64,
                _ => continue,
            };
            props.entry(name).or_insert(numeric);
        }
    }
}

/// Bytes Unreal's compact index spends on `value`.
fn compact_len(value: i64) -> usize {
    let mut n = 1;
    let mut rest = value.unsigned_abs() >> 6;
    while rest > 0 {
        n += 1;
        rest >>= 7;
    }
    n
}

// --- pixel decoding ----------------------------------------------------------

enum DxtAlpha {
    /// DXT1: alpha only via the punch-through palette slot.
    Opaque,
    /// DXT3: 4 bits per pixel, stored before each colour block.
    Explicit,
    /// DXT5: interpolated from two endpoints, stored before each colour block.
    Interpolated,
}

fn decode_dxt(data: &[u8], width: u32, height: u32, alpha: DxtAlpha) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0u8; w * h * 4];
    let block_bytes = match alpha {
        DxtAlpha::Opaque => 8,
        _ => 16,
    };
    for (block_index, block) in data.chunks_exact(block_bytes).enumerate() {
        let bx = block_index % w.div_ceil(4);
        let by = block_index / w.div_ceil(4);
        let colour = &block[block_bytes - 8..];
        let c0 = u16::from_le_bytes([colour[0], colour[1]]);
        let c1 = u16::from_le_bytes([colour[2], colour[3]]);
        let selectors = u32::from_le_bytes([colour[4], colour[5], colour[6], colour[7]]);
        let p0 = rgb565(c0);
        let p1 = rgb565(c1);
        // In DXT1, endpoint order picks between a 4-colour and a 3-colour +
        // transparent palette. DXT3/5 always interpolate both midpoints.
        let four = !matches!(alpha, DxtAlpha::Opaque) || c0 > c1;
        let palette: [[u8; 4]; 4] = if four {
            [
                [p0[0], p0[1], p0[2], 255],
                [p1[0], p1[1], p1[2], 255],
                mix(p0, p1, 2, 1),
                mix(p0, p1, 1, 2),
            ]
        } else {
            [
                [p0[0], p0[1], p0[2], 255],
                [p1[0], p1[1], p1[2], 255],
                mix(p0, p1, 1, 1),
                [0, 0, 0, 0],
            ]
        };
        for py in 0..4 {
            for px in 0..4 {
                let (x, y) = (bx * 4 + px, by * 4 + py);
                if x >= w || y >= h {
                    continue;
                }
                let texel = py * 4 + px;
                let mut rgba = palette[(selectors >> (2 * texel) & 3) as usize];
                match alpha {
                    DxtAlpha::Opaque => {}
                    DxtAlpha::Explicit => {
                        let nibble = block[texel / 2] >> (4 * (texel % 2)) & 0x0F;
                        rgba[3] = nibble * 17;
                    }
                    DxtAlpha::Interpolated => {
                        rgba[3] = dxt5_alpha(&block[..8], texel);
                    }
                }
                out[(y * w + x) * 4..][..4].copy_from_slice(&rgba);
            }
        }
    }
    out
}

fn rgb565(c: u16) -> [u8; 3] {
    [
        ((c >> 11 & 31) * 255 / 31) as u8,
        ((c >> 5 & 63) * 255 / 63) as u8,
        ((c & 31) * 255 / 31) as u8,
    ]
}

fn mix(a: [u8; 3], b: [u8; 3], wa: u16, wb: u16) -> [u8; 4] {
    let c = |i: usize| ((a[i] as u16 * wa + b[i] as u16 * wb) / (wa + wb)) as u8;
    [c(0), c(1), c(2), 255]
}

fn dxt5_alpha(block: &[u8], texel: usize) -> u8 {
    let a0 = block[0] as u16;
    let a1 = block[1] as u16;
    let bits = u64::from_le_bytes([
        block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
    ]);
    let sel = (bits >> (3 * texel) & 7) as u16;
    let value = match (sel, a0 > a1) {
        (0, _) => a0,
        (1, _) => a1,
        (s, true) => ((8 - s) * a0 + (s - 1) * a1) / 7,
        (6, false) => 0,
        (7, false) => 255,
        (s, false) => ((6 - s) * a0 + (s - 1) * a1) / 5,
    };
    value as u8
}

/// Unreal stores RGBA8 textures byte-order blue first.
fn bgra_to_rgba(data: &[u8]) -> Vec<u8> {
    data.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[2], p[1], p[0], p[3]])
        .collect()
}

// --- output ------------------------------------------------------------------

/// Encode as a plain 8-bit RGBA PNG (no filtering — icons are tiny).
pub fn to_png(image: &Rgba) -> Vec<u8> {
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write as _;

    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(payload);
        let mut crc = crc32fast::Hasher::new();
        crc.update(tag);
        crc.update(payload);
        out.extend_from_slice(&crc.finalize().to_be_bytes());
    }

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&image.width.to_be_bytes());
    ihdr.extend_from_slice(&image.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA

    let stride = image.width as usize * 4;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    for row in image.pixels.chunks_exact(stride) {
        encoder.write_all(&[0]).unwrap();
        encoder.write_all(row).unwrap();
    }
    let idat = encoder.finish().unwrap();

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &idat);
    chunk(&mut png, b"IEND", &[]);
    png
}

/// Encode as WebP in libwebp's lossless mode — ~20% smaller than PNG's floor
/// for the atlas's DXT-quantized content even at `near_lossless: 100` (fully
/// lossless), where PNG's filters cannot help (delta-coding destroys the
/// repeated block patterns deflate feeds on). Real libwebp, because the
/// pure-Rust WebP encoders were measured *larger* than the PNG here.
///
/// `near_lossless` is libwebp's preprocessing level: 100 changes nothing
/// (pixel-exact), lower values nudge pixel values where the eye cannot tell,
/// buying another ~15% at 40. The result stays losslessly *coded* either way
/// — this is not DCT lossy mode with its ringing artifacts.
pub fn to_webp(image: &Rgba, near_lossless: u8) -> Result<Vec<u8>, String> {
    let encoder = webp::Encoder::from_rgba(&image.pixels, image.width, image.height);
    // Maximum effort (cwebp's `-z 9`): in lossless mode `quality` spends
    // encoding time, not pixels — at the default effort the file came out
    // *larger* than the PNG.
    let mut config = webp::WebPConfig::new().map_err(|()| "webp config".to_string())?;
    config.lossless = 1;
    config.quality = 100.0;
    config.method = 6;
    config.near_lossless = near_lossless.min(100) as i32;
    // Keep RGB under fully-transparent pixels too — without this libwebp
    // rewrites them for compression, which is invisible but would make even
    // `near_lossless: 100` not byte-for-byte checkable against the PNG.
    config.exact = 1;
    let bytes = encoder
        .encode_advanced(&config)
        .map_err(|e| format!("webp encode: {e:?}"))?;
    Ok(bytes.to_vec())
}

// --- lookup ------------------------------------------------------------------

/// The package file for `package` under `systextures/`, matched
/// case-insensitively the way the client resolves package names.
pub fn find_package(client_dir: &Path, package: &str) -> Result<PathBuf, String> {
    let dir = client_dir.join("systextures");
    let wanted = format!("{package}.utx");
    crate::common::read_dir(&dir)
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(&wanted))
        })
        .ok_or_else(|| format!("{}: no {wanted}", dir.display()))
}

/// Resolve a grp-file reference like `icon.weapon_arcana_mace_i00` to the
/// package file under `systextures/` and the texture name inside it.
pub fn resolve(client_dir: &Path, reference: &str) -> Result<(PathBuf, String), String> {
    let (package, texture) = reference
        .split_once('.')
        .ok_or_else(|| format!("{reference}: expected package.texture"))?;
    Ok((find_package(client_dir, package)?, texture.to_owned()))
}

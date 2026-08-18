//! `AdminPForge` — the packet forge: a GM hand-assembles a server→client
//! packet from an opcode triple and a typed operand list, and sends it to
//! themselves or broadcasts it.
//!
//! It exists to test a client's reaction to a packet the server would not
//! normally produce, so its whole point is to emit bytes no other code path
//! can. That makes it the one admin tool with no game behaviour to match —
//! only a wire format.
//!
//! Three commands: `//forge` opens `pforge/main.htm`; `//forge_values` builds
//! the per-operand editor page from a format string; `//forge_send` assembles
//! and dispatches.
//!
//! **`cs` (client→server) refuses, exactly as Java does.** Java's branch is
//! `throw new UnsupportedOperationException("Not implemented yet!")` with the
//! real body commented out above it — forging *inbound* packets was never
//! shipped upstream. It is refused here for that reason and not because the
//! port deferred it; without this note the refusal reads as a porting gap and
//! invites someone to "finish" a feature that does not exist.

use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::send_to_client;
use commons::network::PacketWriter;

use crate::world::World;

use super::{menu, send_message};

/// Java's `validateOpCodes`: 1-3 opcodes, each bounded by the width its
/// position implies — byte, then word, then dword. That is the client's
/// opcode / ex-opcode / ex-ex-opcode shape.
///
/// Java returns `i > 0` when a token will not parse, i.e. a non-numeric
/// *first* opcode is invalid but a later one merely ends the list. Kept.
fn validate_opcodes(opcodes: &[String]) -> bool {
    if opcodes.is_empty() || opcodes.len() > 3 {
        return false;
    }
    for (i, op) in opcodes.iter().enumerate() {
        let Some(v) = decode_i64(op) else {
            return i > 0;
        };
        if v < 0 {
            return false;
        }
        let max = match i {
            0 => 255,
            1 => 65_535,
            _ => 4_294_967_295,
        };
        if v > max {
            return false;
        }
    }
    true
}

/// `Long.decode` — accepts decimal, `0x`/`#` hex and a leading `0` for octal,
/// with an optional sign. The forge's inputs are hand-typed opcodes, which are
/// conventionally written in hex.
fn decode_i64(s: &str) -> Option<i64> {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if let Some(h) = s.strip_prefix('#') {
        i64::from_str_radix(h, 16).ok()?
    } else if s.len() > 1 && s.starts_with('0') {
        i64::from_str_radix(&s[1..], 8).ok()?
    } else {
        s.parse::<i64>().ok()?
    };
    Some(if neg { -v } else { v })
}

/// Java's `validateFormat`: every char must name an operand type.
fn validate_format(format: &str) -> bool {
    format.chars().all(|c| {
        matches!(
            c.to_ascii_lowercase(),
            'b' | 'x' | 'c' | 'h' | 'd' | 'q' | 'f' | 's'
        )
    })
}

/// `validateMethod` — `sc` send-to-self, `sb` broadcast, `cs` client→server.
fn validate_method(method: &str) -> bool {
    matches!(method, "sc" | "sb" | "cs")
}

/// Java's `write`/`AdminForgePacket.writeImpl`: append one operand of the
/// given type. Returns `false` when the value will not parse as that type,
/// which aborts the whole send (Java throws and falls into its catch).
fn write_operand(w: &mut PacketWriter, ty: char, value: &str) -> bool {
    match ty.to_ascii_lowercase() {
        'c' => match decode_i64(value) {
            Some(v) => {
                w.write_u8(v as u8);
                true
            }
            None => false,
        },
        'h' => match decode_i64(value) {
            Some(v) => {
                w.write_i16(v as i16);
                true
            }
            None => false,
        },
        'd' => match decode_i64(value) {
            Some(v) => {
                w.write_i32(v as i32);
                true
            }
            None => false,
        },
        'q' => match decode_i64(value) {
            Some(v) => {
                w.write_i64(v);
                true
            }
            None => false,
        },
        'f' => match value.parse::<f64>() {
            Ok(v) => {
                w.write_f64(v);
                true
            }
            Err(_) => false,
        },
        // `writeString` — UTF-16LE with a null terminator, which is what
        // `write_string` already emits.
        's' => {
            w.write_string(value);
            true
        }
        // `new BigInteger(string).toByteArray()` — a decimal bignum written as
        // big-endian two's-complement bytes. Java takes the decimal string, so
        // the forge's "array" operand is a number, not a hex blob.
        'b' | 'x' => match big_integer_bytes(value) {
            Some(bytes) => {
                w.write_bytes(&bytes);
                true
            }
            None => false,
        },
        _ => false,
    }
}

/// `new BigInteger(decimal).toByteArray()` for the range this port can hold:
/// the minimal big-endian two's-complement encoding, which for a non-negative
/// value keeps one leading zero byte when the top bit would otherwise be set.
///
/// Java's is arbitrary-precision; this is bounded by `i128`. A GM typing a
/// number wider than that gets the same refusal as any other unparseable
/// operand rather than a silently truncated packet.
fn big_integer_bytes(value: &str) -> Option<Vec<u8>> {
    let v: i128 = value.trim().parse().ok()?;
    if v == 0 {
        return Some(vec![0]);
    }
    let be = v.to_be_bytes();
    // Drop the redundant sign-extension bytes, keeping the first that carries
    // real information plus its sign bit.
    let skip = if v > 0 {
        be.iter().take_while(|&&b| b == 0).count()
    } else {
        be.iter().take_while(|&&b| b == 0xFF).count()
    };
    let mut out = be[skip.min(be.len() - 1)..].to_vec();
    if v > 0 && out[0] & 0x80 != 0 {
        out.insert(0, 0);
    } else if v < 0 && out[0] & 0x80 == 0 {
        out.insert(0, 0xFF);
    }
    Some(out)
}

/// The `$name` substitutions Java resolves before writing an operand: the
/// GM's own state, and their target's.
///
/// `$boid`/`$tboid` are boat object ids. Boats exist in this port (G24.5) but
/// a player's *current* boat is not tracked, so both resolve to Java's
/// no-boat answer, `0` — the same value a GM standing on land would get.
fn substitute(world: &World, gm_oid: i32, token: &str) -> Option<String> {
    let pos = |oid: i32| maybe_position(world, oid);
    let target = || -> Option<i32> {
        world
            .objects
            .get_component::<crate::model::components::TargetRef>(&gm_oid)
            .and_then(|t| t.0)
            .filter(|&t| t != 0)
    };
    let name_of = |oid: i32| {
        world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .map(|p| p.name.clone())
            .or_else(|| {
                world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&oid)
                    .and_then(|n| world.data.npc_data.get(n.npc_id))
                    .map(|t| t.name.clone())
            })
    };
    let s = match token {
        "$oid" => gm_oid.to_string(),
        "$boid" | "$tboid" => "0".to_string(),
        "$title" => world
            .objects
            .get_component::<crate::model::Player>(&gm_oid)
            .map(|p| p.title.clone())
            .unwrap_or_default(),
        "$name" => name_of(gm_oid).unwrap_or_default(),
        "$x" => pos(gm_oid)?.x.to_string(),
        "$y" => pos(gm_oid)?.y.to_string(),
        "$z" => pos(gm_oid)?.z.to_string(),
        "$heading" => pos(gm_oid)?.heading.to_string(),
        "$toid" => target().unwrap_or(0).to_string(),
        "$tname" => target().and_then(name_of).unwrap_or_default(),
        "$ttitle" => target()
            .and_then(|t| {
                world
                    .objects
                    .get_component::<crate::model::Player>(&t)
                    .map(|p| p.title.clone())
            })
            .unwrap_or_default(),
        "$tx" => pos(target()?)?.x.to_string(),
        "$ty" => pos(target()?)?.y.to_string(),
        "$tz" => pos(target()?)?.z.to_string(),
        "$theading" => pos(target()?)?.heading.to_string(),
        _ => return None,
    };
    Some(s)
}

/// Assemble the packet body: the opcodes (typed by position) then each operand.
/// `None` when any piece will not parse, which is Java falling into its catch
/// and re-showing the usage.
fn build_packet(
    world: &World,
    gm_oid: i32,
    opcodes: &[String],
    format: &str,
    values: &[String],
) -> Option<Vec<u8>> {
    let mut w = PacketWriter::new();
    for (i, op) in opcodes.iter().enumerate() {
        let ty = match i {
            0 => 'c',
            1 => 'h',
            _ => 'd',
        };
        if !write_operand(&mut w, ty, op) {
            return None;
        }
    }
    for (i, ty) in format.chars().enumerate() {
        // "Not enough values!"
        let raw = values.get(i)?;
        let resolved = substitute(world, gm_oid, raw).unwrap_or_else(|| raw.clone());
        if !write_operand(&mut w, ty, &resolved) {
            return None;
        }
    }
    Some(w.into_bytes())
}

/// `//forge` — the main page.
pub(super) fn admin_forge(world: &mut World, client_id: u32) {
    menu::show_admin_html(world, client_id, "pforge/main.htm");
}

/// `//forge_values <op1[ op2[ op3]]> ;[ format]` — build the editor page.
///
/// Java's tokenizer reads opcodes until a bare `;`, then optionally a format.
/// The page it renders carries a `%send_bypass%` that calls `//forge_send`
/// back with `$v0…$vN` placeholders the client fills from the input boxes.
pub(super) fn admin_forge_values(world: &mut World, client_id: u32, args: &[&str]) {
    let mut opcodes: Vec<String> = Vec::new();
    let mut rest = args.iter();
    for tok in rest.by_ref() {
        if *tok == ";" {
            break;
        }
        opcodes.push((*tok).to_string());
    }
    if !validate_opcodes(&opcodes) {
        send_message(world, client_id, "Invalid op codes!");
        show_values_usage(world, client_id);
        return;
    }
    let format = rest.next().map(|f| (*f).to_string());
    if let Some(f) = &format
        && !validate_format(f)
    {
        send_message(world, client_id, "Format invalid!");
        show_values_usage(world, client_id);
        return;
    }
    show_values_page(world, client_id, &opcodes, format.as_deref());
}

fn show_values_usage(world: &mut World, client_id: u32) {
    send_message(
        world,
        client_id,
        "Usage: //forge_values opcode1[ opcode2[ opcode3]] ;[ format]",
    );
    admin_forge(world, client_id);
}

fn show_send_usage(world: &mut World, client_id: u32) {
    send_message(
        world,
        client_id,
        "Usage: //forge_send sc|sb|cs opcode1[;opcode2[;opcode3]][ format value1 ... valueN] ",
    );
    admin_forge(world, client_id);
}

/// `showValuesPage`: one editor row per format char, and a bypass that carries
/// the opcodes, the format and a `$vN` placeholder per operand.
fn show_values_page(world: &mut World, client_id: u32, opcodes: &[String], format: Option<&str>) {
    let opformat = match opcodes.len() {
        3 => "chd",
        2 => "ch",
        _ => "c",
    };
    let mut send_bypass = opcodes.join(";");
    let mut editors = String::new();
    if let Some(format) = format {
        send_bypass.push(' ');
        send_bypass.push_str(format);
        let template = crate::data::htm_cache::read_htm_for_client(
            world,
            client_id,
            format!("{}data/html/admin/pforge/inc/editor.htm", world.data.root),
        );
        if let Some(template) = template {
            for (i, ch) in format.chars().enumerate() {
                editors.push_str(
                    &template
                        .replace("%format%", &ch.to_string())
                        .replace("%editor_index%", &i.to_string()),
                );
                send_bypass.push_str(&format!(" $v{i}"));
            }
        }
    }
    menu::show_admin_html_replace(
        world,
        client_id,
        "pforge/values.htm",
        &[
            ("opformat", opformat.to_string()),
            ("opcodes", opcodes.join(";")),
            ("format", format.unwrap_or("").to_string()),
            ("editors", editors),
            ("send_bypass", send_bypass),
        ],
    );
}

/// `//forge_send sc|sb|cs <op1[;op2[;op3]]> [format v1 … vN]`.
pub(super) fn admin_forge_send(world: &mut World, client_id: u32, gm_oid: i32, args: &[&str]) {
    let Some(method) = args.first() else {
        show_send_usage(world, client_id);
        return;
    };
    if !validate_method(method) {
        send_message(world, client_id, "Invalid method!");
        show_send_usage(world, client_id);
        return;
    }
    let Some(op_arg) = args.get(1) else {
        show_send_usage(world, client_id);
        return;
    };
    let opcodes: Vec<String> = op_arg.split(';').map(str::to_string).collect();
    if !validate_opcodes(&opcodes) {
        send_message(world, client_id, "Invalid op codes!");
        show_send_usage(world, client_id);
        return;
    }
    let format = args.get(2).map(|f| (*f).to_string()).unwrap_or_default();
    if !validate_format(&format) {
        send_message(world, client_id, "Format invalid!");
        show_send_usage(world, client_id);
        return;
    }
    let values: Vec<String> = args.iter().skip(3).map(|v| (*v).to_string()).collect();

    // Java: the `cs` body is commented out and the live path throws
    // `UnsupportedOperationException("Not implemented yet!")`. Forging an
    // *inbound* packet was never shipped upstream, so this refuses for the
    // same reason rather than as a port deferral.
    if *method == "cs" {
        send_message(
            world,
            client_id,
            "Method 'cs' (client->server) is not implemented — it is unimplemented in Java too.",
        );
        show_values_page(world, client_id, &opcodes, Some(&format));
        return;
    }

    let Some(bytes) = build_packet(world, gm_oid, &opcodes, &format, &values) else {
        send_message(world, client_id, "Not enough values!");
        show_send_usage(world, client_id);
        return;
    };

    match *method {
        "sc" => {
            send_to_client(world, client_id, bytes);
        }
        // `broadcastPacket` — everyone who can see the GM, the GM included.
        "sb" => crate::game_loop::helpers::broadcast_including_self(world, gm_oid, &bytes),
        _ => unreachable!("validate_method"),
    }
    show_values_page(world, client_id, &opcodes, Some(&format));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opcodes are typed by **position**, so the same number is fine as the
    /// second opcode and out of range as the first.
    #[test]
    fn opcode_ranges_are_per_position() {
        assert!(validate_opcodes(&["0x2F".into()]));
        assert!(!validate_opcodes(&["256".into()]), "first is a byte");
        assert!(
            validate_opcodes(&["0x2F".into(), "256".into()]),
            "second is a word"
        );
        assert!(!validate_opcodes(&["0x2F".into(), "65536".into()]));
        assert!(
            validate_opcodes(&["1".into(), "2".into(), "65536".into()]),
            "third is a dword"
        );
        assert!(!validate_opcodes(&[]), "at least one");
        assert!(
            !validate_opcodes(&["1".into(), "2".into(), "3".into(), "4".into()]),
            "at most three"
        );
        assert!(!validate_opcodes(&["-1".into()]), "never negative");
    }

    /// `Long.decode`'s radix prefixes — opcodes are conventionally hex.
    #[test]
    fn opcodes_decode_hex_octal_and_decimal() {
        assert_eq!(decode_i64("0x2F"), Some(47));
        assert_eq!(decode_i64("#2F"), Some(47));
        assert_eq!(decode_i64("047"), Some(39)); // octal
        assert_eq!(decode_i64("47"), Some(47));
        assert_eq!(decode_i64("-0x10"), Some(-16));
        assert_eq!(decode_i64("nonsense"), None);
    }

    /// Every format char writes the width the client expects, little-endian.
    #[test]
    fn each_operand_type_writes_its_own_width() {
        let out = |ty: char, v: &str| {
            let mut w = PacketWriter::new();
            assert!(write_operand(&mut w, ty, v), "{ty} {v}");
            w.into_bytes()
        };
        assert_eq!(out('c', "0x2F"), vec![0x2F]);
        assert_eq!(out('h', "0x1234"), vec![0x34, 0x12]);
        assert_eq!(out('d', "1"), vec![1, 0, 0, 0]);
        assert_eq!(out('q', "1"), vec![1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(out('f', "1.0"), 1.0f64.to_le_bytes().to_vec());
        // UTF-16LE + null terminator.
        assert_eq!(out('s', "Hi"), vec![b'H', 0, b'i', 0, 0, 0]);
        // Uppercase is the same type.
        assert_eq!(out('D', "1"), out('d', "1"));
    }

    /// The `b`/`x` operand is Java's `new BigInteger(decimal).toByteArray()` —
    /// big-endian, minimal, sign-extended. A GM typing `255` gets two bytes,
    /// not one, because the top bit would otherwise read as negative.
    #[test]
    fn the_array_operand_is_a_bigendian_bignum() {
        assert_eq!(big_integer_bytes("0"), Some(vec![0]));
        assert_eq!(big_integer_bytes("1"), Some(vec![1]));
        assert_eq!(big_integer_bytes("127"), Some(vec![0x7F]));
        assert_eq!(big_integer_bytes("255"), Some(vec![0x00, 0xFF]));
        assert_eq!(big_integer_bytes("-1"), Some(vec![0xFF]));
        assert_eq!(big_integer_bytes("256"), Some(vec![0x01, 0x00]));
    }

    /// A format char with no value left is Java's "Not enough values!", not a
    /// short packet quietly put on the wire.
    #[test]
    fn a_missing_value_aborts_the_whole_packet() {
        let (world, ..) = crate::game_loop::tests::admin_world();
        assert!(build_packet(&world, 1, &["0x2F".into()], "dd", &["1".into()]).is_none());
        assert!(
            build_packet(&world, 1, &["0x2F".into()], "dd", &["1".into(), "2".into()]).is_some()
        );
    }

    #[test]
    fn format_and_method_validation_matches_java() {
        assert!(validate_format("chdqfsbx"));
        assert!(validate_format("CHDQFSBX"));
        assert!(validate_format(""), "an empty format is allowed");
        assert!(!validate_format("z"));
        for m in ["sc", "sb", "cs"] {
            assert!(validate_method(m));
        }
        assert!(!validate_method("xx"));
    }
}

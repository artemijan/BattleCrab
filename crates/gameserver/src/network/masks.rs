//! Port of `serverpackets/AbstractMaskPacket` — masked packets flag which
//! component blocks are present with one bit per component, in **reversed**
//! bit order: component mask 0 is the *high* bit (0x80) of byte 0. Getting
//! this order wrong desyncs the client (see PROGRESS.md cross-cutting notes).

/// Java `DEFAULT_FLAG_ARRAY`.
pub const DEFAULT_FLAG_ARRAY: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

/// Java `addMask` — set the bit for component `mask`.
pub fn add_mask(masks: &mut [u8], mask: usize) {
    masks[mask >> 3] |= DEFAULT_FLAG_ARRAY[mask & 7];
}

/// Java `containsMask` — is the bit for component `mask` set?
pub fn contains_mask(masks: &[u8], mask: usize) -> bool {
    masks[mask >> 3] & DEFAULT_FLAG_ARRAY[mask & 7] != 0
}

/// Build an `N`-byte mask with the given component masks set
/// (Java `addComponentType` over `values()`).
pub fn build_mask<const N: usize>(components: impl IntoIterator<Item = usize>) -> [u8; N] {
    let mut masks = [0u8; N];
    for mask in components {
        add_mask(&mut masks, mask);
    }
    masks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_bit_order() {
        assert_eq!(build_mask::<3>([0]), [0x80, 0, 0]);
        assert_eq!(build_mask::<3>([7]), [0x01, 0, 0]);
        assert_eq!(build_mask::<3>([8]), [0, 0x80, 0]);
        // All 23 UserInfo blocks → the byte-verified UserInfo mask.
        assert_eq!(build_mask::<3>(0..23), [0xFF, 0xFF, 0xFE]);
        // All 33 equip slots → slot 32 is the high bit of byte 4.
        assert_eq!(build_mask::<5>(0..33), [0xFF, 0xFF, 0xFF, 0xFF, 0x80]);
    }

    #[test]
    fn contains_matches_add() {
        let mut masks = [0u8; 5];
        add_mask(&mut masks, 14);
        add_mask(&mut masks, 32);
        assert!(contains_mask(&masks, 14));
        assert!(contains_mask(&masks, 32));
        assert!(!contains_mask(&masks, 15));
    }
}

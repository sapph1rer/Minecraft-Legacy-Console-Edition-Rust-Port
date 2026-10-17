//! Plain replacements for 4J's `CompressedTileStorage`, `SparseDataStorage` and
//! `SparseLightStorage`.
//!
//! The originals exist to save memory on 256 MB consoles. Their *observable* behaviour is just
//! "a 16×128×16 array of bytes / nibbles, addressed `x << 11 | z << 7 | y`", which is what is
//! kept here. Memory is saved lazily instead: a section that is entirely at its default value
//! (air, data 0, block light 0, sky light 0 or 15) owns no allocation at all.

/// Height of one storage section (`Level::COMPRESSED_CHUNK_SECTION_HEIGHT`).
pub const SECTION_HEIGHT: usize = 128;
/// Blocks in a section (`COMPRESSED_CHUNK_SECTION_TILES`).
pub const SECTION_TILES: usize = 16 * 16 * SECTION_HEIGHT;

/// Index of (x, y, z) inside a section; `y` is relative to the section.
#[inline]
pub const fn index(x: usize, y: usize, z: usize) -> usize {
    (x << 11) | (z << 7) | y
}

/// 4-bit values packed two to a byte, in the same order as Java's `DataLayer`
/// (even index = low nibble).
#[derive(Clone, Debug)]
pub struct Nibbles {
    data: Option<Box<[u8; SECTION_TILES / 2]>>,
    fill: u8,
}

impl Nibbles {
    /// All values equal `fill` (0..=15) until written.
    pub fn new(fill: u8) -> Nibbles {
        debug_assert!(fill < 16);
        Nibbles { data: None, fill }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        match &self.data {
            None => self.fill,
            Some(d) => {
                let i = index(x, y, z);
                let b = d[i >> 1];
                if i & 1 == 0 {
                    b & 15
                } else {
                    b >> 4
                }
            }
        }
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, v: u8) {
        let v = v & 15;
        if self.data.is_none() {
            if v == self.fill {
                return;
            }
            self.data = Some(Box::new([self.fill | (self.fill << 4); SECTION_TILES / 2]));
        }
        let d = self.data.as_mut().expect("allocated above");
        let i = index(x, y, z);
        let b = &mut d[i >> 1];
        if i & 1 == 0 {
            *b = (*b & 0xf0) | v;
        } else {
            *b = (*b & 0x0f) | (v << 4);
        }
    }

    /// `setAllBright` and friends: every value becomes `v`.
    pub fn fill_all(&mut self, v: u8) {
        self.data = None;
        self.fill = v & 15;
    }

    /// Exactly 16384 bytes in Java `DataLayer` order.
    pub fn to_bytes(&self) -> Vec<u8> {
        match &self.data {
            None => vec![self.fill | (self.fill << 4); SECTION_TILES / 2],
            Some(d) => d.to_vec(),
        }
    }

    pub fn from_bytes(bytes: &[u8], default_fill: u8) -> Nibbles {
        assert_eq!(bytes.len(), SECTION_TILES / 2, "a nibble section is 16384 bytes");
        let mut n = Nibbles::new(default_fill);
        let mut arr = Box::new([0u8; SECTION_TILES / 2]);
        arr.copy_from_slice(bytes);
        n.data = Some(arr);
        n
    }

    /// True when no memory is allocated (everything still equals the fill value).
    pub fn is_uniform(&self) -> bool {
        self.data.is_none()
    }
}

/// One 128-high half of a chunk column.
#[derive(Clone, Debug)]
pub struct Section {
    blocks: Option<Box<[u8; SECTION_TILES]>>,
    pub data: Nibbles,
    pub sky: Nibbles,
    pub block_light: Nibbles,
}

impl Section {
    /// An upper section (y 128..255) with nothing in it: air, open sky above.
    pub fn empty_upper() -> Section {
        Section { blocks: None, data: Nibbles::new(0), sky: Nibbles::new(15), block_light: Nibbles::new(0) }
    }

    /// A lower section with nothing in it yet. `SparseLightStorage(true)` starts with the
    /// top plane (y = 127) fully lit and everything below dark; `recalcHeightmap` fills the rest.
    pub fn empty_lower() -> Section {
        let mut s = Section { blocks: None, data: Nibbles::new(0), sky: Nibbles::new(0), block_light: Nibbles::new(0) };
        for x in 0..16 {
            for z in 0..16 {
                s.sky.set(x, SECTION_HEIGHT - 1, z, 15);
            }
        }
        s
    }

    #[inline]
    pub fn block(&self, x: usize, y: usize, z: usize) -> u8 {
        match &self.blocks {
            None => 0,
            Some(b) => b[index(x, y, z)],
        }
    }

    #[inline]
    pub fn set_block(&mut self, x: usize, y: usize, z: usize, v: u8) {
        if self.blocks.is_none() {
            if v == 0 {
                return;
            }
            self.blocks = Some(Box::new([0u8; SECTION_TILES]));
        }
        self.blocks.as_mut().expect("allocated above")[index(x, y, z)] = v;
    }

    /// Replace all blocks with `src` (32768 bytes, `x << 11 | z << 7 | y`).
    pub fn set_blocks(&mut self, src: &[u8]) {
        assert_eq!(src.len(), SECTION_TILES);
        if src.iter().all(|&b| b == 0) {
            self.blocks = None;
        } else {
            let mut arr = Box::new([0u8; SECTION_TILES]);
            arr.copy_from_slice(src);
            self.blocks = Some(arr);
        }
    }

    pub fn blocks_to_vec(&self) -> Vec<u8> {
        match &self.blocks {
            None => vec![0; SECTION_TILES],
            Some(b) => b.to_vec(),
        }
    }

    pub fn blocks_slice(&self) -> Option<&[u8]> {
        self.blocks.as_deref().map(|b| &b[..])
    }

    /// Highest y (relative) holding a non-air block, if any (`getHighestNonEmptyY`).
    pub fn highest_non_empty_y(&self) -> Option<usize> {
        let b = self.blocks.as_ref()?;
        let mut best: Option<usize> = None;
        for col in 0..256 {
            let base = col << 7;
            for y in (0..SECTION_HEIGHT).rev() {
                if b[base + y] != 0 {
                    best = Some(best.map_or(y, |v| v.max(y)));
                    break;
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibbles_roundtrip_and_lazy_allocation() {
        let mut n = Nibbles::new(15);
        assert!(n.is_uniform());
        assert_eq!(n.get(3, 4, 5), 15);
        n.set(3, 4, 5, 15); // same as fill: still no allocation
        assert!(n.is_uniform());
        n.set(3, 4, 5, 7);
        assert!(!n.is_uniform());
        assert_eq!(n.get(3, 4, 5), 7);
        assert_eq!(n.get(3, 5, 5), 15, "neighbour nibble keeps the fill value");
        assert_eq!(n.get(3, 3, 5), 15);
        n.set(3, 5, 5, 2);
        assert_eq!(n.get(3, 4, 5), 7);
        assert_eq!(n.get(3, 5, 5), 2);
    }

    #[test]
    fn java_nibble_order() {
        let mut n = Nibbles::new(0);
        // index 0 -> low nibble of byte 0, index 1 -> high nibble of byte 0
        n.set(0, 0, 0, 0xA);
        n.set(0, 1, 0, 0xB);
        let bytes = n.to_bytes();
        assert_eq!(bytes[0], 0xBA);
        let back = Nibbles::from_bytes(&bytes, 0);
        assert_eq!(back.get(0, 0, 0), 0xA);
        assert_eq!(back.get(0, 1, 0), 0xB);
    }

    #[test]
    fn section_blocks() {
        let mut s = Section::empty_lower();
        assert_eq!(s.block(1, 2, 3), 0);
        s.set_block(1, 2, 3, 0); // no allocation for air
        assert!(s.blocks_slice().is_none());
        s.set_block(1, 2, 3, 42);
        assert_eq!(s.block(1, 2, 3), 42);
        assert_eq!(s.blocks_slice().unwrap()[index(1, 2, 3)], 42);
        assert_eq!(s.highest_non_empty_y(), Some(2));
        // lower section starts with the top plane lit
        assert_eq!(s.sky.get(0, 127, 0), 15);
        assert_eq!(s.sky.get(0, 126, 0), 0);
        assert_eq!(Section::empty_upper().sky.get(7, 0, 7), 15);
    }
}

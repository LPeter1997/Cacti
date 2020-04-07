//! Implementation of the DEFLATE decompression based on RFC 1951.

use std::io::{Result, Error, ErrorKind, Read};
use std::mem::MaybeUninit;
use std::hash::{Hasher, BuildHasherDefault};
use crate::BitReader;

// HASHER //////////////////////////////////////////////////////////////////////

struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> FnvHasher {
        FnvHasher(0xcbf29ce484222325)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let FnvHasher(mut hash) = *self;

        for byte in bytes.iter() {
            hash = hash ^ (*byte as u64);
            hash = hash.wrapping_mul(0x100000001b3);
        }

        *self = FnvHasher(hash);
    }
}

type FnvBuildHasher = BuildHasherDefault<FnvHasher>;

//type HashMap<K, V> = std::collections::HashMap<K, V, FnvBuildHasher>;
type HashMap<K, V> = std::collections::HashMap<K, V>;

////////////////////////////////////////////////////////////////////////////////

/// The number of bytes the DEFLATE algorithm can reference back.
const WINDOW_LENGTH: usize = 32768;
/// The maximum number of bits the DEFLATE algorithm accepts as a Huffman-code.
const MAX_CODE_BITS: usize = 15;

/// A fixed-size sliding window implementation using a circular buffer. This is
/// to store the last 32 KiB for backreferences.
struct SlidingWindow {
    buffer: Box<[u8; WINDOW_LENGTH]>,
    cursor: usize,
}

impl std::fmt::Debug for SlidingWindow {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // TODO
        unimplemented!();
    }
}

impl SlidingWindow {
    /// Creates a new `SlidingWindow`.
    fn new() -> Self {
        Self{
            buffer: unsafe { Box::new(MaybeUninit::uninit().assume_init()) },
            cursor: 0,
        }
    }

    /// Adds an element to the `SlidingWindow`.
    fn push(&mut self, element: u8) {
        self.buffer[self.cursor] = element;
        // Slide
        self.cursor = (self.cursor + 1) % self.buffer.len();
    }

    // NOTE: This could be optimized for some cases but it's not that trivial to
    // do so.
    /// Adds a slice to the `SlidingWindow`.
    fn push_slice(&mut self, elements: &[u8]) {
        for e in elements {
            self.push(*e);
        }
    }

    /// Returns the buffer index corresponding to the given dostance from the
    /// cursor.
    fn buffer_index_of_dist(&self, dist: isize) -> usize {
        ((self.cursor as isize + dist) as usize) % self.buffer.len()
    }

    /// Returns the element the given distance away from the cursor.
    fn peek(&self, dist: isize) -> u8 {
        let idx = self.buffer_index_of_dist(dist);
        self.buffer[idx]
    }

    /// Copies the back-referenced slice into the buffer and returns the newly
    /// inserted region as a pair of slices.
    ///
    /// This implementation only goes for correctness, no optimizations are
    /// performed.
    fn backreference_trivial(&mut self, dist: isize, len: usize) -> (&[u8], &[u8]) {
        let start = self.cursor;
        for _ in 0..len {
            let e = self.peek(dist);
            self.push(e);
        }
        let end = self.cursor;
        // Determine if we need to split
        if end <= start {
            (&self.buffer[start..], &self.buffer[..end])
        }
        else {
            (&self.buffer[start..end], &self.buffer[0..0])
        }
    }

    // NOTE: This could be optimized further for some cases but it's not that
    // trivial to do so.
    /// Copies the back-referenced slice into the buffer and returns the newly
    /// inserted region as a pair of slices.
    fn backreference(&mut self, dist: isize, len: usize) -> (&[u8], &[u8]) {
        /*let start_copy = self.buffer_index_of_dist(dist);
        let end_copy = (start_copy + len) % self.buffer.len();

        if     self.cursor + len <= self.buffer.len()
            && start_copy <= end_copy
            && (start_copy >= self.cursor + len || end_copy < self.cursor) {

            // Trivial memcopy
            let src = self.buffer[start_copy..].as_ptr();
            let dst = self.buffer[self.cursor..].as_mut_ptr();
            unsafe { std::ptr::copy_nonoverlapping(src, dst, len) };
            let result_slice = &self.buffer[self.cursor..(self.cursor + len)];
            self.cursor = (self.cursor + len) % self.buffer.len();
            return (result_slice, &self.buffer[0..0]);
        }*/

        // Fallback
        self.backreference_trivial(dist, len)
    }
}

/// State for a non-compressed DEFLATE block.
#[derive(Debug)]
struct NonCompressed {
    /// Overall size of the block.
    size: usize,
    // What has been copied so far from it.
    copied: usize,
}

/// Description of a backreference for LZ77.
#[derive(Debug, Clone, Copy)]
struct Backref {
    /// The length of the copy.
    length: usize,
    /// The distance to jump back.
    distance: isize,
}

/// State for a Huffman-encoded DEFLATE block.
#[derive(Debug)]
struct Huffman {
    /// The literal and length code dictionary.
    lit_len: HashMap<u16, u16>,
    /// The distance-code dictionary.
    dist: HashMap<u16, u16>,
    /// The currently processed backreference.
    backref: Option<Backref>,
}

/// State for the currently decompressed DEFLATE block.
#[derive(Debug)]
enum DeflateBlock {
    NonCompressed(NonCompressed),
    Huffman(Huffman),
}

/// A type for implementing the DEFLATE decompression algorithm.
#[derive(Debug)]
pub struct Deflate<R: Read> {
    reader: BitReader<R>,
    is_last_block: bool,
    current_block: Option<DeflateBlock>,
    window: SlidingWindow,
}

impl <R:  Read> Deflate<R> {
    /// Creates a new `Deflate` structure from the given `Read` type.
    pub fn new(reader: R) -> Self {
        Self{
            reader: BitReader::new(reader),
            is_last_block: false,
            current_block: None,
            // NOTE: We could lazily allocate this when needed
            window: SlidingWindow::new(),
        }
    }

    /// Generates canonic Huffman-codes from code lengths that are all
    /// left-padded with a `1`, so we don't have to store code-lengths for
    /// leading `0`s.
    /// Taken from RFC 1951, 3.2.2.
    fn generate_huffman(lens: &[usize]) -> Result<HashMap<u16, u16>> {
        // Find the max length
        let max_bits = lens.iter().cloned().max().unwrap_or(0);
        if max_bits > MAX_CODE_BITS {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid code length!"));
        }
        // Count the number of codes for each code length
        let mut bl_count = [0u16; MAX_CODE_BITS + 1];
        for l in lens {
            bl_count[*l] += 1;
        }
        // Find the numerical value of the smallest code for each code length
        let mut next_code = vec![0u16; MAX_CODE_BITS + 1];
        let mut code = 0u16;
        // Setting to 1 instead of 0 to make the extra padding bit on the left
        bl_count[0] = 1;
        for bits in 1..=max_bits {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }
        // Allocate codes
        let mut result = HashMap::default();
        for n in 0..lens.len() {
            let len = lens[n];
            if len != 0 {
                let code = next_code[len];
                result.insert(code, n as u16);
                next_code[len] += 1;
            }
        }
        Ok(result)
    }

    // Header reading //////////////////////////////////////////////////////////

    /// Reads in a non-compressed block header, returning the `NonCompressed`
    /// descriptor for it.
    /// RFC 3.2.4.
    fn read_not_compressed_header(&mut self) -> Result<NonCompressed> {
        let len = self.reader.read_aligned_le_u16()?;
        let nlen = self.reader.read_aligned_le_u16()?;
        if len != !nlen {
            return Err(Error::new(ErrorKind::InvalidData, "LEN != ~NLEN"));
        }
        Ok(NonCompressed{ size: len as usize, copied: 0 })
    }

    /// Reads in a fixed Huffman-encoded header, returning the `Huffman`
    /// descriptor for it.
    /// RFC 3.2.6.
    fn read_fixed_huffman_header(&mut self) -> Result<Huffman> {
        // Literal and length codes
        let mut litlen_lens = [0usize; 288];
        for i in 000..=143 { litlen_lens[i] = 8; }
        for i in 144..=255 { litlen_lens[i] = 9; }
        for i in 256..=279 { litlen_lens[i] = 7; }
        for i in 280..=287 { litlen_lens[i] = 8; }
        let lit_len = Self::generate_huffman(&litlen_lens)?;
        // Distance codes
        let dist_lens = [5usize; 32];
        let dist = Self::generate_huffman(&dist_lens)?;
        Ok(Huffman{
            lit_len,
            dist,
            backref: None,
        })
    }

    /// Reads in a dynamic Huffman-encoded header, returning the `Huffman`
    /// descriptor for it.
    /// RFC 3.2.7.
    fn read_dynamic_huffman_header(&mut self)  -> Result<Huffman> {
        let n_litlen = self.reader.read_to_u8(5)? as usize + 257;
        let n_dist = self.reader.read_to_u8(5)? as usize + 1;
        let n_codelen_codes = self.reader.read_to_u8(4)? as usize + 4;

        // Code length code lengths
        let mut codelen_codelens = [0usize; 19];
        codelen_codelens[16] = self.reader.read_to_u8(3)? as usize;
        codelen_codelens[17] = self.reader.read_to_u8(3)? as usize;
        codelen_codelens[18] = self.reader.read_to_u8(3)? as usize;
        codelen_codelens[00] = self.reader.read_to_u8(3)? as usize;
        for i in 0..(n_codelen_codes - 4) {
            // A dank formula
            let mul = 1 - (((i as isize % 2) * 2) as isize);
            let idx = (8 + (i as isize + 1) / 2 * mul) as usize;
            codelen_codelens[idx] = self.reader.read_to_u8(3)? as usize;
        }

        // Construct the code-length code
        let codelen_code = Self::generate_huffman(&codelen_codelens)?;

        // We decode all code-lengths in one go, as their processing is the same
        // NOTE: We could just have an array here
        let mut all_codelens = vec![0usize; n_litlen + n_dist];
        let mut codelen_idx = 0;
        while codelen_idx < all_codelens.len() {
            let sym = self.decode_huffman_symbol(&codelen_code)? as usize;
            if sym < 16 {
                // Simple length
                all_codelens[codelen_idx] = sym;
                codelen_idx += 1;
                continue;
            }
            // Repetition
            let mut sym_to_repeat = 0;
            let repeat = match sym {
                16 => {
                    if codelen_idx == 0 {
                        return Err(Error::new(ErrorKind::InvalidData, "No code length to repeat!"));
                    }
                    sym_to_repeat = all_codelens[codelen_idx - 1];
                    self.reader.read_to_u8(2)? as usize + 3
                },
                17 => self.reader.read_to_u8(3)? as usize + 3,
                18 => self.reader.read_to_u8(7)? as usize + 11,
                _ => return Err(Error::new(ErrorKind::InvalidData, "Illegal code length symbol!")),
            };
            for _ in 0..repeat {
                all_codelens[codelen_idx] = sym_to_repeat;
                codelen_idx += 1;
            }
        }

        // Construct lit-len codes
        let litlen_codelens = &all_codelens[0..n_litlen];
        let lit_len = Self::generate_huffman(litlen_codelens)?;

        // Construct distance codes, but also check RFC stuff
        let dist_codelens = &all_codelens[n_litlen..];
        let dist = if dist_codelens.len() == 1 && dist_codelens[0] == 0 {
            // Just literals
            HashMap::default()
        }
        else {
            let mut one = 0;
            let mut more = 0;
            for d in dist_codelens.iter() {
                if *d == 1 {
                    one += 1;
                }
                else if *d > 1 {
                    more += 1;
                }
            }
            // If one distance is defined, complete the tree
            if one == 1 && more == 0 {
                // We have one unused code
                let mut dist_codelens = dist_codelens.to_vec();
                dist_codelens.resize(32, 0);
                dist_codelens[31] = 1;
                Self::generate_huffman(&dist_codelens)?
            }
            else {
                Self::generate_huffman(dist_codelens)?
            }
        };

        Ok(Huffman{
            lit_len,
            dist,
            backref: None,
        })
    }

    /// Reads in a block header, returning the `DeflateBlock` that describes it.
    /// RFC 3.2.3.
    fn read_block_header(&mut self) -> Result<DeflateBlock> {
        self.is_last_block = self.reader.read_bit()? != 0;
        let btype = self.reader.read_to_u8(2)?;
        match btype {
            0b00 => Ok(DeflateBlock::NonCompressed(self.read_not_compressed_header()?)),
            0b01 => Ok(DeflateBlock::Huffman(self.read_fixed_huffman_header()?)),
            0b10 => Ok(DeflateBlock::Huffman(self.read_dynamic_huffman_header()?)),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid block type!")),
        }
    }

    // Reading non-compressed blocks ///////////////////////////////////////////

    /// Reads in a non-compressed block to fill the given buffer as much as
    /// possible. Returns a tuple of filled bytes and `true`, if the block has
    /// ended.
    fn read_non_compressed(&mut self, buf: &mut [u8], state: &mut NonCompressed) -> Result<(usize, bool)> {
        let rem = state.size - state.copied;
        let can_read = std::cmp::min(rem, buf.len());
        self.reader.read_aligned_to_buffer(&mut buf[..can_read])?;
        self.window.push_slice(&buf[..can_read]);
        state.copied += can_read;
        Ok((can_read, state.size == state.copied))
    }

    // Decoding Huffman-encoded blocks /////////////////////////////////////////

    /// Decodes a Huffman symbol from the given dictionary.
    fn decode_huffman_symbol(&mut self, dict: &HashMap<u16, u16>) -> Result<u16> {
        let mut code = 1u16;
        for _ in 0..MAX_CODE_BITS {
            code = (code << 1) | self.reader.read_bit()? as u16;
            if let Some(sym) = dict.get(&code) {
                return Ok(*sym);
            }
        }
        Err(Error::new(ErrorKind::InvalidData, "Invalid symbol code!"))
    }

    /// Decodes the repetition length from the literal-length symbol.
    /// RFC 3.2.5.
    fn decode_huffman_length(&mut self, lit_len: u16) -> Result<usize> {
        match lit_len {
            x @ 257..=264 => Ok((x - 257 + 3) as usize),
            x @ 265..=284 => {
                let x0 = x - 265;
                let increment = 2 << (x0 / 4);
                let p2 = (1 << (x0 / 4 + 3)) - 8;
                let len = (x0 % 4 * increment + p2 + 11) as usize;
                let extra = (x0 / 4) + 1;
                let extra = self.reader.read_to_u8(extra as usize)? as usize;
                Ok(len + extra)
            },
            285 => Ok(258),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid length symbol!")),
        }
    }

    /// Decodes the repetition distance from the distance symbol.
    /// RFC 3.2.5.
    fn decode_huffman_distance(&mut self, sym: u16) -> Result<usize> {
        match sym {
            x @ 0..=3 => Ok((x + 1) as usize),
            x @ 4..=29 => {
                let x0 = x - 4;
                let increment = 2 << (x0 / 2);
                let p2 = (1 << (x0 / 2 + 2)) - 4;
                let len = (x0 % 2 * increment + p2 + 5) as usize;
                let extra = (x0 / 2) + 1;
                let extra = self.reader.read_to_u16(extra as usize)? as usize;
                Ok(len + extra)
            },
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid distance symbol!")),
        }
    }

    /// Reads in a Huffman-encoded block to fill the given buffer as much as
    /// possible. Returns a tuple of filled bytes and `true`, if the block has
    /// ended.
    fn read_huffman(&mut self, buf: &mut [u8], state: &mut Huffman) -> Result<(usize, bool)> {
        let mut filled = 0;
        loop {
            // Check if we have read enough
            if filled >= buf.len() {
                return Ok((filled, false));
            }
            // Check if we have copies to do
            if let Some(mut backref) = state.backref {
                // Determine the most we can read
                let rem_buf = &mut buf[filled..];
                let can_read = std::cmp::min(backref.length, rem_buf.len());
                // Copy that amount
                let (w1, w2) = self.window.backreference(backref.distance, can_read);
                rem_buf[..w1.len()].copy_from_slice(w1);
                let rem_buf = &mut rem_buf[w1.len()..];
                rem_buf[..w2.len()].copy_from_slice(w2);
                // We advanced that amount with the read
                backref.length -= can_read;
                filled += can_read;
                // If the len is 0, we are done copying
                if backref.length == 0 {
                    state.backref = None;
                }
                else {
                    state.backref = Some(backref);
                }
                continue;
            }
            // We need to read a symbol
            let sym = self.decode_huffman_symbol(&state.lit_len)?;
            // Check if end of block
            if sym == 256 {
                return Ok((filled, true));
            }
            // Not end of block
            if sym < 256 {
                // Simple symbol
                buf[filled] = sym as u8;
                self.window.push(sym as u8);
                filled += 1;
                continue;
            }
            // Length and distance code
            // We decode the length from the already read symbol, since
            // their dict. are unified
            let length = self.decode_huffman_length(sym)?;
            // Get distance symbol
            let dist_sym = self.decode_huffman_symbol(&state.dist)?;
            // Decode the the distance symbol
            let distance = -(self.decode_huffman_distance(dist_sym)? as isize);
            // Add it to the state
            state.backref = Some(Backref{ length, distance });
        }
    }
}

impl <R: Read> Read for Deflate<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut filled = 0;
        loop {
            // Check if we need to read more
            if filled == buf.len() {
                return Ok(filled);
            }
            // Check if we need to read in a block
            if self.current_block.is_none() {
                // If was the last block, we are done
                if self.is_last_block {
                    return Ok(filled);
                }
                // We need to read in the next block
                self.current_block = Some(self.read_block_header()?);
            }
            // We must have some block here
            assert!(self.current_block.is_some());
            let mut block = self.current_block.take();
            let (read, is_over) = match block.as_mut().unwrap() {
                DeflateBlock::NonCompressed(nc) =>
                    self.read_non_compressed(&mut buf[filled..], nc)?,
                DeflateBlock::Huffman(huffman) =>
                    self.read_huffman(&mut buf[filled..], huffman)?,
            };
            filled += read;
            if is_over {
                block = None;
            }
            self.current_block = block;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Test both SlidingWindow and Deflate
}

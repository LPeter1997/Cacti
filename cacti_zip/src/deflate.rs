//! Implementation of the DEFLATE decompression based on RFC 1951.

use std::io::{Result, Error, ErrorKind, Read};
use std::mem::MaybeUninit;
use std::collections::HashMap;

// ////////////////////////////////////////////////////////////////////////// //
//                              Bitwise reading                               //
// ////////////////////////////////////////////////////////////////////////// //

/// The number of bytes the `BitReader` keeps in the cache.
const BIT_READER_CACHE_SIZE: usize = 4;

/// A bitwise adapter for readers for processing data on non-byte boundlaries.
#[derive(Debug)]
struct BitReader<R: Read> {
    reader: R,
    cache: [u8; BIT_READER_CACHE_SIZE],
    bit_index: usize,
}

impl <R: Read> BitReader<R> {
    /// Creates a new `BitReader` from the given reader.
    fn new(reader: R) -> Self {
        Self {
            reader,
            cache: [0u8; BIT_READER_CACHE_SIZE],
            bit_index: BIT_READER_CACHE_SIZE * 8,
        }
    }

    /// Makes sure to have the maximum number of unread elements in the cache
    /// possible.
    fn ensure_cache(&mut self) -> Result<()> {
        if self.bit_index < 8 {
            // Cache is full with unread bytes
            return Ok(());
        }
        // We can throw away some bytes, also need to read as many
        let can_read = self.bit_index / 8;
        let keep = BIT_READER_CACHE_SIZE - can_read;
        // Shift back the elements we keep
        for i in 0..keep {
            self.cache[i] = self.cache[i + can_read];
        }
        // Make sure our bit-index now points into the first byte
        self.bit_index %= 8;
        // Read into the extra space
        self.reader.read(&mut self.cache[keep..])?;
        Ok(())
    }

    /// Returns the cache reinterpreted as an `u32`.
    fn cache_as_u32(&mut self) -> u32 {
        unsafe { std::mem::transmute(self.cache) }
    }

    /// Peeks the bit at the given offset without consuming any of the input.
    fn peek_bit(&mut self, offset: usize) -> Result<u8> {
       self.ensure_cache()?;
       let result = ((self.cache_as_u32() >> (self.bit_index + offset)) & 1) as u8;
       Ok(result)
    }

    /// Reads the next bit, consuming it.
    fn read_bit(&mut self) -> Result<u8> {
        let result = self.peek_bit(0)?;
        self.bit_index += 1;
        Ok(result)
    }

    /// Peeks bits, returning them in a `u8`.
    fn peek_to_u8(&mut self, count: usize) -> Result<u8> {
        const MASKS: [u32; 9] = [
            0b00000000, 0b00000001, 0b00000011, 0b00000111, 0b00001111,
            0b00011111, 0b00111111, 0b01111111, 0b11111111,
        ];
        self.ensure_cache()?;
        let result = ((self.cache_as_u32() >> self.bit_index) & MASKS[count]) as u8;
        Ok(result)
    }

    /// Reads bits, returning them in a `u8`.
    fn read_to_u8(&mut self, count: usize) -> Result<u8> {
        let result = self.peek_to_u8(count)?;
        self.bit_index += count;
        Ok(result)
    }

    /// Peeks bits, returning them in a `u16`.
    fn peek_to_u16(&mut self, count: usize) -> Result<u16> {
        const MASKS: [u32; 17] = [
            0b0000000000000000, 0b0000000000000001, 0b0000000000000011,
            0b0000000000000111, 0b0000000000001111, 0b0000000000011111,
            0b0000000000111111, 0b0000000001111111, 0b0000000011111111,
            0b0000000111111111, 0b0000001111111111, 0b0000011111111111,
            0b0000111111111111, 0b0001111111111111, 0b0011111111111111,
            0b0111111111111111, 0b1111111111111111,
        ];
        self.ensure_cache()?;
        let result = ((self.cache_as_u32() >> self.bit_index) & MASKS[count]) as u16;
        Ok(result)
    }

    /// Reads bits, returning them in a `u16`.
    fn read_to_u16(&mut self, count: usize) -> Result<u16> {
        let result = self.peek_to_u16(count)?;
        self.bit_index += count;
        Ok(result)
    }

    /// Consumes the given amount of bits.
    fn consume_bits(&mut self, count: usize) {
        self.bit_index += count;
    }

    /// Skips to the next byte boundlary.
    fn skip_to_byte(&mut self) {
        self.bit_index += (8 - self.bit_index % 8) % 8;
    }

    /// Reads a little-endian `u16` aligned to bytes.
    fn read_aligned_le_u16(&mut self) -> Result<u16> {
        self.skip_to_byte();
        self.ensure_cache()?;
        let result = u16::from_le_bytes([self.cache[0], self.cache[1]]);
        self.bit_index += 16;
        Ok(result)
    }

    /// Tries to fill the given buffer to full capacity.
    fn read_aligned_to_buffer(&mut self, buffer: &mut [u8]) -> Result<()> {
        self.skip_to_byte();
        self.ensure_cache()?;
        if buffer.len() <= BIT_READER_CACHE_SIZE {
            // No extra reads
            buffer.copy_from_slice(&self.cache[..buffer.len()]);
            self.bit_index += buffer.len() * 8;
            Ok(())
        }
        else {
            // Full cache invalidation, extra reads
            buffer[..BIT_READER_CACHE_SIZE].copy_from_slice(&self.cache);
            // Extra read
            self.reader.read_exact(&mut buffer[BIT_READER_CACHE_SIZE..])?;
            self.bit_index = BIT_READER_CACHE_SIZE * 8;
            Ok(())
        }
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Huffman codes                                //
// ////////////////////////////////////////////////////////////////////////// //

/// Reverses the bits of an `u8`.
fn reverse_u8_bits(n: u8) -> u8 {
    const LUT: [u8; 16] = [
        0x0, 0x8, 0x4, 0xC, 0x2, 0xA, 0x6, 0xE,
        0x1, 0x9, 0x5, 0xD, 0x3, 0xB, 0x7, 0xF,
    ];
    // Low 4 bits
    let lo4 = LUT[(n & 0b1111) as usize];
    // High 4 bits
    let hi4 = LUT[(n >> 4) as usize];
    // Reassemble
    (lo4 << 4) | hi4
}

/// Reverses the bits of an `u16`.
fn reverse_u16_bits(n: u16) -> u16 {
      ((reverse_u8_bits((n & 0xff) as u8) as u16) << 8)
    | reverse_u8_bits((n >> 8) as u8) as u16
}

/// Generates canonical Huffman-codes from the given code-lengths. The codes are
/// passed to the given callback.
fn generate_huffman_from_lengths<F>(code_lens: &[usize], mut f: F)
    where F: FnMut(u16, HuffmanCode) {
    // Count code length occurrences
    let mut bl_count = [0u16; 16];
    for len in code_lens {
        bl_count[*len] += 1;
    }
    // Determine base value for each code length
    let mut next_code = [0u16; 16];
    let mut code = 0u16;
    bl_count[0] = 0;
    for bits in 1..=DEFLATE_MAX_BITS {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    // Assign each symbol a unique code based on the base values
    for symbol in 0..code_lens.len() {
        let length = code_lens[symbol];
        if length != 0 {
            // Allocate a new code for it
            let code = next_code[length];
            let desc = HuffmanCode{ symbol: symbol as u16, length };
            f(code, desc);
            next_code[length] += 1;
        }
    }
}

/// The maximum number of bits a Huffman-code allows for LUT optimizations.
const HUFFMAN_LUT_BITS: usize = 10;
/// The symbol value that's considered an invalid entry.
const HUFFMAN_INVALID_SYMBOL: u16 = 999;

// NOTE: Instead of invalid symbols the LUT could somehow aid the search for the
// given symbol. This is possible because a subset of codes must start with the
// searched bits.

// NOTE: We don't need `usize` for code-lengths, an `u8` is enough.

/// Represents a single Huffman-code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HuffmanCode {
    symbol: u16,
    length: usize,
}

/// Represents a helper-structure for canonical Huffman-codes that implements
/// LUT optimization for short codes.
struct HuffmanCodes {
    lut: Box<[HuffmanCode; 1 << HUFFMAN_LUT_BITS]>,
    dict: HashMap<u16, HuffmanCode>,
}

impl std::fmt::Debug for HuffmanCodes {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // TODO
        unimplemented!();
    }
}

impl HuffmanCodes {
    /// Creates a new, empty `HuffmanCodes` structure.
    fn new() -> Self {
        let mut lut: Box<[HuffmanCode; 1 << HUFFMAN_LUT_BITS]> =
            Box::new(unsafe { MaybeUninit::uninit().assume_init() });
        for entry in lut.iter_mut() {
            entry.symbol = HUFFMAN_INVALID_SYMBOL;
            entry.length = 0;
        }
        Self {
            lut,
            dict: HashMap::new(),
        }
    }

    /// Creates a `HuffmanCodes` structure from the given code-lengths.
    fn from_code_lengths(code_lens: &[usize]) -> Self {
        let mut result = Self::new();
        generate_huffman_from_lengths(code_lens, |mut code, desc| {
            // We reverse each code as the function generates them in the spec order
            code = reverse_u16_bits(code) >> (16 - desc.length);
            if desc.length <= HUFFMAN_LUT_BITS {
                // We LUT optimize it
                // We need to generate all bit-combinations that fit behind this
                // code
                let gen_bits = HUFFMAN_LUT_BITS - desc.length;
                for i in 0..(1 << gen_bits) {
                    // Assemble the full bit-stream
                    let index = (i << desc.length) | code;
                    result.lut[index as usize] = desc;
                    //println!("LUT {:0>width$b} [{}] -> {}", index, desc.length, desc.symbol, width=HUFFMAN_LUT_BITS);
                }
            }
            else {
                // Goes into the dictionary
                // NOTE: Do we need to 1-pad this???
                //println!("{:0>width$b} -> {}", code, desc.symbol, width=desc.length);
                //result.dict.insert(code | 0b1000000000000000, desc.symbol);
                result.dict.insert(code, desc);
            }
        });
        result
    }

    /// Decodes a Huffman-code from the given `BitReader`.
    fn decode_symbol<R: Read>(&self, r: &mut BitReader<R>) -> Result<u16> {
        // First we try the LUT-way
        let bits = r.peek_to_u16(HUFFMAN_LUT_BITS)?;
        let desc = &self.lut[bits as usize];
        if desc.symbol != HUFFMAN_INVALID_SYMBOL {
            // Found it
            r.consume_bits(desc.length);
            return Ok(desc.symbol);
        }
        // Not found, try in the dictionary
        //unimplemented!();
        //let mut code = bits | 0b1000000000000000;
        let mut code = bits;
        for i in HUFFMAN_LUT_BITS..DEFLATE_MAX_BITS {
            code = code | ((r.peek_bit(i)? as u16) << i);
            if let Some(desc) = self.dict.get(&code) {
                // Found it
                if desc.length == i + 1 {
                    r.consume_bits(i + 1);
                    return Ok(desc.symbol);
                }
            }
        }
        // Not found
        return Err(Error::new(ErrorKind::InvalidData, "No such code!"));
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Sliding window                               //
// ////////////////////////////////////////////////////////////////////////// //

/// The maximum number of bytes the DEFLATE algorithm can reference back.
const DEFLATE_WINDOW_SIZE: usize = 32768;

/// A fixed-size sliding window implementation using a circular buffer. This is
/// to store the last 32 KiB for backreferences.
struct SlidingWindow {
    buffer: Box<[u8; DEFLATE_WINDOW_SIZE]>,
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

// ////////////////////////////////////////////////////////////////////////// //
//                           Deflate implementation                           //
// ////////////////////////////////////////////////////////////////////////// //

/// The maximum number of bits the DEFLATE spec allows a code-length to be.
const DEFLATE_MAX_BITS: usize = 15;

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
    lit_len: HuffmanCodes,
    /// The distance-code dictionary.
    dist: HuffmanCodes,
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
        let lit_len = HuffmanCodes::from_code_lengths(&litlen_lens);
        // Distance codes
        let dist_lens = [5usize; 32];
        let dist = HuffmanCodes::from_code_lengths(&dist_lens);
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
        let codelen_code = HuffmanCodes::from_code_lengths(&codelen_codelens);

        // We decode all code-lengths in one go, as their processing is the same
        // NOTE: We could just have an array here
        let mut all_codelens = vec![0usize; n_litlen + n_dist];
        let mut codelen_idx = 0;
        while codelen_idx < all_codelens.len() {
            let sym = codelen_code.decode_symbol(&mut self.reader)?;
            if sym < 16 {
                // Simple length
                all_codelens[codelen_idx] = sym as usize;
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
        let lit_len = HuffmanCodes::from_code_lengths(litlen_codelens);

        // Construct distance codes, but also check RFC stuff
        let dist_codelens = &all_codelens[n_litlen..];
        let dist = if dist_codelens.len() == 1 && dist_codelens[0] == 0 {
            // Just literals
            HuffmanCodes::new()
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
                HuffmanCodes::from_code_lengths(&dist_codelens)
            }
            else {
                HuffmanCodes::from_code_lengths(dist_codelens)
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
            let sym = state.lit_len.decode_symbol(&mut self.reader)?;
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
            let dist_sym = state.dist.decode_symbol(&mut self.reader)?;
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

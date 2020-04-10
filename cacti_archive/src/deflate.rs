//! Implementation of the DEFLATE format based on RFC 1951.

use std::io::{Result, Error, ErrorKind, Read};
use std::mem::MaybeUninit;
use std::hash::{Hasher, BuildHasherDefault};
use std::fmt;

// ////////////////////////////////////////////////////////////////////////// //
//                                  FNV Hash                                  //
// ////////////////////////////////////////////////////////////////////////// //

// Reference:
// https://en.wikipedia.org/wiki/Fowler%E2%80%93Noll%E2%80%93Vo_hash_function#FNV-1a_hash

struct FnvHasher(u64);

impl Default for FnvHasher {
    #[inline(always)]
    fn default() -> FnvHasher {
        FnvHasher(0xcbf29ce484222325)
    }
}

impl Hasher for FnvHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = self.0;
        for byte in bytes {
            hash = hash ^ (*byte as u64);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.0 = hash;
    }
}

type FnvBuildHasher = BuildHasherDefault<FnvHasher>;
type HashMap<K, V> = std::collections::HashMap<K, V, FnvBuildHasher>;

// ////////////////////////////////////////////////////////////////////////// //
//                              Bitwise reading                               //
// ////////////////////////////////////////////////////////////////////////// //

/// The number of bytes the `BitReader` keeps in the cache.
const BIT_READER_CACHE_SIZE: usize = 8;

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
    #[inline(always)]
    fn ensure_cache(&mut self, minbits: usize) -> Result<()> {
        if self.bit_index < minbits {
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
    #[inline(always)]
    fn cache_as_u64(&mut self) -> u64 {
        unsafe { std::mem::transmute(self.cache) }
    }

    /// Peeks the bit at the given offset without consuming any of the input.
    #[inline(always)]
    fn peek_bit(&mut self, offset: usize) -> Result<u8> {
       self.ensure_cache(64)?;
       let result = ((self.cache_as_u64() >> (self.bit_index + offset)) & 1) as u8;
       Ok(result)
    }

    /// Reads the next bit, consuming it.
    #[inline(always)]
    fn read_bit(&mut self) -> Result<u8> {
        let result = self.peek_bit(0)?;
        self.bit_index += 1;
        Ok(result)
    }

    /// Peeks bits, returning them in a `u8`.
    #[inline(always)]
    fn peek_to_u8(&mut self, count: usize) -> Result<u8> {
        const MASKS: [u64; 9] = [
            0b00000000, 0b00000001, 0b00000011, 0b00000111, 0b00001111,
            0b00011111, 0b00111111, 0b01111111, 0b11111111,
        ];
        self.ensure_cache(56)?;
        let result = ((self.cache_as_u64() >> self.bit_index) & MASKS[count]) as u8;
        Ok(result)
    }

    /// Reads bits, returning them in a `u8`.
    #[inline(always)]
    fn read_to_u8(&mut self, count: usize) -> Result<u8> {
        let result = self.peek_to_u8(count)?;
        self.bit_index += count;
        Ok(result)
    }

    /// Peeks bits, returning them in a `u16`.
    #[inline(always)]
    fn peek_to_u16(&mut self, count: usize) -> Result<u16> {
        const MASKS: [u64; 17] = [
            0b0000000000000000, 0b0000000000000001, 0b0000000000000011,
            0b0000000000000111, 0b0000000000001111, 0b0000000000011111,
            0b0000000000111111, 0b0000000001111111, 0b0000000011111111,
            0b0000000111111111, 0b0000001111111111, 0b0000011111111111,
            0b0000111111111111, 0b0001111111111111, 0b0011111111111111,
            0b0111111111111111, 0b1111111111111111,
        ];
        self.ensure_cache(48)?;
        let result = ((self.cache_as_u64() >> self.bit_index) & MASKS[count]) as u16;
        Ok(result)
    }

    /// Reads bits, returning them in a `u16`.
    #[inline(always)]
    fn read_to_u16(&mut self, count: usize) -> Result<u16> {
        let result = self.peek_to_u16(count)?;
        self.bit_index += count;
        Ok(result)
    }

    /// Consumes the given amount of bits.
    #[inline(always)]
    fn consume_bits(&mut self, count: usize) {
        self.bit_index += count;
    }

    /// Skips to the next byte boundlary.
    #[inline(always)]
    fn skip_to_byte(&mut self) {
        self.bit_index += (8 - self.bit_index % 8) % 8;
    }

    /// Reads a little-endian `u16` aligned to bytes.
    #[inline(always)]
    fn read_aligned_le_u16(&mut self) -> Result<u16> {
        self.skip_to_byte();
        self.ensure_cache(8)?;
        let result = u16::from_le_bytes([self.cache[0], self.cache[1]]);
        self.bit_index += 16;
        Ok(result)
    }

    /// Tries to fill the given buffer to full capacity.
    #[inline(always)]
    fn read_aligned_to_buffer(&mut self, buffer: &mut [u8]) -> Result<()> {
        self.skip_to_byte();
        self.ensure_cache(8)?;
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
#[inline(always)]
fn reverse_u8_bits(n: u8) -> u8 {
    const LUT: [u8; 256] = [
        0x00, 0x80, 0x40, 0xc0, 0x20, 0xa0, 0x60, 0xe0, 0x10, 0x90, 0x50, 0xd0,
        0x30, 0xb0, 0x70, 0xf0, 0x08, 0x88, 0x48, 0xc8, 0x28, 0xa8, 0x68, 0xe8,
        0x18, 0x98, 0x58, 0xd8, 0x38, 0xb8, 0x78, 0xf8, 0x04, 0x84, 0x44, 0xc4,
        0x24, 0xa4, 0x64, 0xe4, 0x14, 0x94, 0x54, 0xd4, 0x34, 0xb4, 0x74, 0xf4,
        0x0c, 0x8c, 0x4c, 0xcc, 0x2c, 0xac, 0x6c, 0xec, 0x1c, 0x9c, 0x5c, 0xdc,
        0x3c, 0xbc, 0x7c, 0xfc, 0x02, 0x82, 0x42, 0xc2, 0x22, 0xa2, 0x62, 0xe2,
        0x12, 0x92, 0x52, 0xd2, 0x32, 0xb2, 0x72, 0xf2, 0x0a, 0x8a, 0x4a, 0xca,
        0x2a, 0xaa, 0x6a, 0xea, 0x1a, 0x9a, 0x5a, 0xda, 0x3a, 0xba, 0x7a, 0xfa,
        0x06, 0x86, 0x46, 0xc6, 0x26, 0xa6, 0x66, 0xe6, 0x16, 0x96, 0x56, 0xd6,
        0x36, 0xb6, 0x76, 0xf6, 0x0e, 0x8e, 0x4e, 0xce, 0x2e, 0xae, 0x6e, 0xee,
        0x1e, 0x9e, 0x5e, 0xde, 0x3e, 0xbe, 0x7e, 0xfe, 0x01, 0x81, 0x41, 0xc1,
        0x21, 0xa1, 0x61, 0xe1, 0x11, 0x91, 0x51, 0xd1, 0x31, 0xb1, 0x71, 0xf1,
        0x09, 0x89, 0x49, 0xc9, 0x29, 0xa9, 0x69, 0xe9, 0x19, 0x99, 0x59, 0xd9,
        0x39, 0xb9, 0x79, 0xf9, 0x05, 0x85, 0x45, 0xc5, 0x25, 0xa5, 0x65, 0xe5,
        0x15, 0x95, 0x55, 0xd5, 0x35, 0xb5, 0x75, 0xf5, 0x0d, 0x8d, 0x4d, 0xcd,
        0x2d, 0xad, 0x6d, 0xed, 0x1d, 0x9d, 0x5d, 0xdd, 0x3d, 0xbd, 0x7d, 0xfd,
        0x03, 0x83, 0x43, 0xc3, 0x23, 0xa3, 0x63, 0xe3, 0x13, 0x93, 0x53, 0xd3,
        0x33, 0xb3, 0x73, 0xf3, 0x0b, 0x8b, 0x4b, 0xcb, 0x2b, 0xab, 0x6b, 0xeb,
        0x1b, 0x9b, 0x5b, 0xdb, 0x3b, 0xbb, 0x7b, 0xfb, 0x07, 0x87, 0x47, 0xc7,
        0x27, 0xa7, 0x67, 0xe7, 0x17, 0x97, 0x57, 0xd7, 0x37, 0xb7, 0x77, 0xf7,
        0x0f, 0x8f, 0x4f, 0xcf, 0x2f, 0xaf, 0x6f, 0xef, 0x1f, 0x9f, 0x5f, 0xdf,
        0x3f, 0xbf, 0x7f, 0xff,
    ];
    LUT[n as usize]
}

/// Reverses the bits of an `u16`.
#[inline(always)]
fn reverse_u16_bits(n: u16) -> u16 {
      ((reverse_u8_bits((n & 0xff) as u8) as u16) << 8)
    | reverse_u8_bits((n >> 8) as u8) as u16
}

/// Generates canonical Huffman-codes from the given code-lengths. The codes are
/// passed to the given callback.
#[inline(always)]
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
    dict: HashMap<u16, u16>,
}

impl fmt::Debug for HuffmanCodes {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("HuffmanCodes")
            // TODO: lut field
            // TODO: dict field
            .finish()
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
            dict: HashMap::default(),
        }
    }

    /// Creates a `HuffmanCodes` structure from the given code-lengths.
    #[inline(always)]
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
                }
            }
            else {
                // Goes into the dictionary
                // 1-pad it
                code |= 1 << desc.length;
                result.dict.insert(code, desc.symbol);
            }
        });
        result
    }

    /// Decodes a Huffman-code from the given `BitReader`.
    #[inline(always)]
    fn decode_symbol<R: Read>(&self, r: &mut BitReader<R>) -> Result<u16> {
        const ONE_PADS: [u16; 15] = [
            0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400,
            0x800, 0x1000, 0x2000, 0x4000, 0x8000
        ];
        // First we try the LUT-way
        let bits = r.peek_to_u16(HUFFMAN_LUT_BITS)?;
        let desc = &self.lut[bits as usize];
        if desc.symbol != HUFFMAN_INVALID_SYMBOL {
            // Found it
            r.consume_bits(desc.length);
            return Ok(desc.symbol);
        }
        // Not found, try in the dictionary
        let mut code = bits;
        for i in HUFFMAN_LUT_BITS..DEFLATE_MAX_BITS {
            code = code | ((r.peek_bit(i)? as u16) << i);
            // 1-pad it
            let real_code = code | ONE_PADS[i];
            if let Some(symbol) = self.dict.get(&real_code) {
                // Found it
                r.consume_bits(i + 1);
                return Ok(*symbol);
            }
        }
        // Not found
        Err(Error::new(ErrorKind::InvalidData, "No such code!"))
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
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("SlidingWindow")
            // TODO: buffer field
            // TODO: cursor field
            .finish()
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
    #[inline(always)]
    fn push(&mut self, element: u8) {
        self.buffer[self.cursor] = element;
        // Slide
        self.cursor = (self.cursor + 1) % self.buffer.len();
    }

    // NOTE: This could be optimized for some cases but it's not that trivial to
    // do so.
    /// Adds a slice to the `SlidingWindow`.
    #[inline(always)]
    fn push_slice(&mut self, elements: &[u8]) {
        let copy_end = self.cursor + elements.len();
        if copy_end <= self.buffer.len() {
            // Trivial forward-copy
            self.buffer[self.cursor..copy_end].copy_from_slice(elements);
            self.cursor = copy_end;
        }
        else {
            // Copy in 2 pieces
            let part1_len = self.buffer.len() - self.cursor;
            let cursor_end = copy_end % self.buffer.len();
            self.buffer[self.cursor..].copy_from_slice(&elements[..part1_len]);
            self.buffer[..cursor_end].copy_from_slice(&elements[part1_len..]);
            self.cursor = cursor_end;
        }
    }

    /// Returns the buffer index corresponding to the given dostance from the
    /// cursor.
    #[inline(always)]
    fn buffer_index_of_dist(&self, dist: isize) -> usize {
        ((self.cursor as isize + dist) as usize) % self.buffer.len()
    }

    /// Returns the element the given distance away from the cursor.
    #[inline(always)]
    fn peek(&self, dist: isize) -> u8 {
        let idx = self.buffer_index_of_dist(dist);
        self.buffer[idx]
    }

    /// Copies the back-referenced slice into the buffer and returns the newly
    /// inserted region as a pair of slices.
    ///
    /// This implementation only goes for correctness, no optimizations are
    /// performed.
    #[inline(always)]
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

    /// Copies the back-referenced slice using memcpy. This assumes no overlaps
    /// or wraps happen.
    #[inline(always)]
    fn backreference_memcopy(&mut self, start: usize, len: usize) -> (&[u8], &[u8]) {
        // Trivial memcopy
        let src = self.buffer[start..].as_ptr();
        let dst = self.buffer[self.cursor..].as_mut_ptr();
        unsafe { std::ptr::copy_nonoverlapping(src, dst, len) };
        let result_slice = &self.buffer[self.cursor..(self.cursor + len)];
        self.cursor = (self.cursor + len) % self.buffer.len();
        (result_slice, &self.buffer[0..0])
    }

    /// Copies the back-referenced slice with a left-to-right bytewise copy.
    /// This assumes no wraps happen.
    #[inline(always)]
    fn backreference_bytecopy(&mut self, start: usize, len: usize) -> (&[u8], &[u8]) {
        for i in 0..len {
            self.buffer[self.cursor + i] = self.buffer[start + i];
        }
        let result_slice = &self.buffer[self.cursor..(self.cursor + len)];
        self.cursor = (self.cursor + len) % self.buffer.len();
        (result_slice, &self.buffer[0..0])
    }

    /// Does backreference by memset-ting a single byte.
    #[inline(always)]
    fn backreference_memset(&mut self, byte: u8, len: usize) -> (&[u8], &[u8]) {
        let dst = self.buffer[self.cursor..].as_mut_ptr();
        unsafe { std::ptr::write_bytes(dst, byte, len) };
        let result_slice = &self.buffer[self.cursor..(self.cursor + len)];
        self.cursor = (self.cursor + len) % self.buffer.len();
        (result_slice, &self.buffer[0..0])
    }

    // NOTE: This could be optimized further for some cases but it's not that
    // trivial to do so.
    /// Copies the back-referenced slice into the buffer and returns the newly
    /// inserted region as a pair of slices.
    #[inline(always)]
    fn backreference(&mut self, dist: isize, len: usize) -> (&[u8], &[u8]) {
        let start_copy = self.buffer_index_of_dist(dist);
        let end_copy = (start_copy + len) % self.buffer.len();

        let cursor_nowrap = self.cursor + len <= self.buffer.len();
        let src_no_wrap = start_copy <= end_copy;

        if cursor_nowrap && src_no_wrap {
            let no_overlap = start_copy >= self.cursor + len || end_copy < self.cursor;
            if no_overlap {
                // Trivial memcopy
                return self.backreference_memcopy(start_copy, len);
            }
            // At least we don't need to wrap-check stuff
            let forward_copy = !(start_copy > self.cursor);
            if forward_copy {
                let overlap = self.cursor - start_copy;
                if overlap == 1 {
                    // Byte-wise memset
                    let byte = self.buffer[start_copy];
                    return self.backreference_memset(byte, len);
                }
            }

            return self.backreference_bytecopy(start_copy, len);
        }

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

// NOTE: We could store codes in the `Deflate` structure to avoid reallocation.
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
pub struct Inflate<R: Read> {
    reader: BitReader<R>,
    is_last_block: bool,
    current_block: Option<DeflateBlock>,
    window: SlidingWindow,
}

impl <R:  Read> Inflate<R> {
    /// Creates a new `Inflate` structure from the given reader.
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

    // NOTE: We could pre-generate this.
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
        const OFFSETS: [usize; 20] = [
            11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51,
            59, 67, 83, 99, 115, 131, 163, 195, 227
        ];
        match lit_len {
            x @ 257..=264 => Ok((x - 257 + 3) as usize),
            x @ 265..=284 => {
                let extra = (x - 261) / 4;
                let offset = OFFSETS[(x - 265) as usize];
                let extra = self.reader.read_to_u8(extra as usize)? as usize;
                Ok(offset + extra)
            },
            285 => Ok(258),
            _ => Err(Error::new(ErrorKind::InvalidData, "Invalid length symbol!")),
        }
    }

    /// Decodes the repetition distance from the distance symbol.
    /// RFC 3.2.5.
    fn decode_huffman_distance(&mut self, sym: u16) -> Result<usize> {
        const OFFSETS: [usize; 26] = [
            5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513,
            769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577
        ];
        match sym {
            x @ 0..=3 => Ok((x + 1) as usize),
            x @ 4..=29 => {
                let extra = (x - 2) / 2;
                let offset = OFFSETS[(x - 4) as usize];
                let extra = self.reader.read_to_u16(extra as usize)? as usize;
                Ok(offset + extra)
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

impl <R: Read> Read for Inflate<R> {
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

    // TODO: Test both SlidingWindow and Inflate
}

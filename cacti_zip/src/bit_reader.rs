//! A general-purpose bitwise reader.

// NOTE: Not that general purpose anymore, we started optimizing it for DEFLATE.

use std::io::{Result, Read};

/// The number of bytes the `BitReader` keeps for looking ahead.
const LOOKAHEAD_SIZE: usize = 4;

/// A bitwise reader for streams that require non-byte aligned reads.
#[derive(Debug)]
pub struct BitReader<R: Read> {
    reader: R,
    lookahead: [u8; LOOKAHEAD_SIZE],
    bit_index: usize,
}

impl <R: Read> BitReader<R> {
    /// Creates a new `BitReader` from the given underlying `Read` type.
    pub fn new(reader: R) -> Self {
        Self{
            reader,
            lookahead: [0, 0, 0, 0],
            bit_index: LOOKAHEAD_SIZE * 8,
        }
    }

    /// Ensures to always have `LOOKAHEAD_SIZE` peeked in the buffer.
    #[inline(always)]
    fn ensure_peek(&mut self) -> Result<()> {
        if self.bit_index < 8 {
            // We have all the bytes peeked already
            return Ok(());
        }
        // We have at least one byte to read
        let to_read = self.bit_index / 8;
        let keep = LOOKAHEAD_SIZE - to_read;
        // Shift back existing bytes
        for i in 0..keep {
            self.lookahead[i] = self.lookahead[i + to_read];
        }
        // Read in to the remaining places
        self.reader.read(&mut self.lookahead[keep..])?;
        // Wrap cursor
        self.bit_index %= 8;
        Ok(())
    }

    /// Returns the peek buffer as an `u32`.
    #[inline(always)]
    fn peek_buffer_as_u32(&mut self) -> u32 {
        unsafe { std::mem::transmute(self.lookahead) }
    }

    /// Reads the next bit from the stream. Either `1` or `0`.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    #[inline(always)]
    pub fn read_bit(&mut self) -> Result<u8> {
        self.ensure_peek()?;
        let result = ((self.peek_buffer_as_u32() >> self.bit_index) & 1) as u8;
        self.bit_index += 1;
        Ok(result)
    }

    /// Peeks multiple bits without consumption, assembling it into an `u8`.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    #[inline(always)]
    pub fn peek_to_u8(&mut self, count: usize) -> Result<u8> {
        const MASKS: [u32; 9] = [
            0b00000000,
            0b00000001, 0b00000011, 0b00000111, 0b00001111,
            0b00011111, 0b00111111, 0b01111111, 0b11111111,
        ];
        self.ensure_peek()?;
        let result = ((self.peek_buffer_as_u32() >> self.bit_index) & MASKS[count]) as u8;
        Ok(result)
    }

    /// Reads in multiple bits into an `u8`.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    #[inline(always)]
    pub fn read_to_u8(&mut self, count: usize) -> Result<u8> {
        let result = self.peek_to_u8(count)?;
        self.bit_index += count;
        Ok(result)
    }

    /// Reads in multiple bits into an `u16`.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    #[inline(always)]
    pub fn read_to_u16(&mut self, count: usize) -> Result<u16> {
        const MASKS: [u32; 17] = [
            0b0000000000000000,
            0b0000000000000001, 0b0000000000000011, 0b0000000000000111, 0b0000000000001111,
            0b0000000000011111, 0b0000000000111111, 0b0000000001111111, 0b0000000011111111,
            0b0000000111111111, 0b0000001111111111, 0b0000011111111111, 0b0000111111111111,
            0b0001111111111111, 0b0011111111111111, 0b0111111111111111, 0b1111111111111111,
        ];
        self.ensure_peek()?;
        let result = ((self.peek_buffer_as_u32() >> self.bit_index) & MASKS[count]) as u16;
        self.bit_index += count;
        Ok(result)
    }

    /// Skips to the start of next byte. If already on a byte-boundlary, this is
    /// a no-op.
    #[inline(always)]
    pub fn skip_to_byte(&mut self) {
        let to_skip = (8 - self.bit_index % 8) % 8;
        self.bit_index += to_skip;
    }

    /// Consumes the given number of bits.
    #[inline(always)]
    pub fn consume_bits(&mut self, count: usize) {
        self.bit_index += count;
    }

    /// Reads in an aligned `u8`, skipping the remaining of the current byte.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    #[inline(always)]
    pub fn read_aligned_u8(&mut self) -> Result<u8> {
        self.ensure_peek()?;
        if self.bit_index == 0 {
            self.bit_index = 8;
            Ok(self.lookahead[0])
        }
        else {
            self.bit_index = 16;
            Ok(self.lookahead[1])
        }
    }

    /// Reads in an aligned `u16`, skipping the remaining of the current byte.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    #[inline(always)]
    pub fn read_aligned_le_u16(&mut self) -> Result<u16> {
        self.ensure_peek()?;
        if self.bit_index == 0 {
            self.bit_index = 16;
            Ok(u16::from_le_bytes([self.lookahead[0], self.lookahead[1]]))
        }
        else {
            self.bit_index = 24;
            Ok(u16::from_le_bytes([self.lookahead[1], self.lookahead[2]]))
        }
    }

    /// Reads in the exact amount of aligned bytes into the given buffer,
    /// skipping the remaining of the current byte.
    ///
    /// # Errors
    ///
    /// In case of an IO error or unfilled buffer, an error variant is returned.
    #[inline(always)]
    pub fn read_aligned_to_buffer(&mut self, buffer: &mut [u8]) -> Result<()> {
        self.skip_to_byte();
        self.ensure_peek()?;
        if buffer.len() > LOOKAHEAD_SIZE {
            // Read LOOKAHEAD_SIZE bytes from the lookahead
            buffer[..LOOKAHEAD_SIZE].copy_from_slice(&self.lookahead);
            self.reader.read_exact(&mut buffer[LOOKAHEAD_SIZE..])?;
            self.bit_index = LOOKAHEAD_SIZE * 8;
            Ok(())
        }
        else {
            buffer.copy_from_slice(&self.lookahead[..buffer.len()]);
            self.bit_index = 8 * buffer.len();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader() -> Result<()> {
        let bytes = [0b10110101, 0b01110110, 0b00101011, 0b11000101];
        let bytes: &[u8] = &bytes;
        let mut r = BitReader::new(bytes);
        r.skip_to_byte(); // Must be no-op
        assert_eq!(r.read_bit()?, 1);
        assert_eq!(r.read_bit()?, 0);
        assert_eq!(r.read_to_u8(4)?, 0b1101);
        assert_eq!(r.read_to_u8(5)?, 0b11010);
        assert_eq!(r.read_to_u8(4)?, 0b1110);
        r.skip_to_byte();  // Must be no-op
        assert_eq!(r.read_to_u8(3)?, 0b011);
        r.skip_to_byte();
        assert_eq!(r.read_to_u8(4)?, 0b0101);
        Ok(())
    }
}

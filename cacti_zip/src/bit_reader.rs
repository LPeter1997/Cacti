//! A general-purpose bitwise reader.

use std::io::{Result, Error, ErrorKind, Read};

// NOTE: This could be useful in some io_utils library.

/// A bitwise reader for streams that require non-byte aligned reads.
#[derive(Debug)]
pub struct BitReader<R: Read> {
    reader: R,
    current_byte: u8,
    bit_index: u8,
}

impl <R: Read> BitReader<R> {
    /// Creates a new `BitReader` from the given underlying `Read` type.
    pub fn new(reader: R) -> Self {
        Self{
            reader,
            current_byte: 0,
            bit_index: 8,
        }
    }

    /// Reads the next bit from the stream. Either `1` or `0`.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    pub fn read_bit(&mut self) -> Result<u8> {
        if self.bit_index == 8 {
            // Read next byte
            self.bit_index = 0;
            let mut bs: [u8; 1] = [0];
            self.reader.read_exact(&mut bs)?;
            self.current_byte = bs[0];
        }
        // Get bit
        let bit = (self.current_byte >> self.bit_index) & 0b1;
        self.bit_index += 1;
        Ok(bit)
    }

    /// Reads in multiple bits into an `u8`.
    ///
    /// # Errors
    ///
    /// In case of an IO error or a `count` greater than `8`, an error variant
    /// is returned.
    pub fn read_to_u8(&mut self, count: usize) -> Result<u8> {
        if count > 8 {
            return Err(Error::new(ErrorKind::InvalidInput, "Can't read > 8 bits into an u8!"));
        }
        let mut result = 0u8;
        for i in 0..count {
            result |= self.read_bit()? << i;
        }
        Ok(result)
    }

    /// Skips to the start of next byte. If already on a byte-boundlary, this is
    /// a no-op.
    pub fn skip_to_byte(&mut self) {
        self.bit_index = 8;
    }

    /// Reads in an aligned `u8`, skipping the remaining of the current byte.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    pub fn read_aligned_u8(&mut self) -> Result<u8> {
        let mut b: [u8; 1] = [0];
        self.read_aligned_to_buffer(&mut b)?;
        Ok(b[0])
    }

    /// Reads in an aligned `u16`, skipping the remaining of the current byte.
    ///
    /// # Errors
    ///
    /// In case of an IO error, an error variant is returned.
    pub fn read_aligned_le_u16(&mut self) -> Result<u16> {
        let mut bs: [u8; 2] = [0, 0];
        self.read_aligned_to_buffer(&mut bs)?;
        Ok(u16::from_le_bytes(bs))
    }

    /// Reads in the exact amount of aligned bytes into the given buffer,
    /// skipping the remaining of the current byte.
    ///
    /// # Errors
    ///
    /// In case of an IO error or unfilled buffer, an error variant is returned.
    pub fn read_aligned_to_buffer(&mut self, buffer: &mut [u8]) -> Result<()> {
        self.skip_to_byte();
        self.reader.read_exact(buffer)?;
        Ok(())
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

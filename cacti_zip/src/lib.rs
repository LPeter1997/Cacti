
use std::path::Path;
use std::io::{Read, Seek, SeekFrom};
use std::collections::HashMap;
use std::fs;
use std::io;

/// The internal reader.
#[derive(Debug)]
struct ByteReader<R: Read + Seek> {
    reader: R,
    length: usize,
    offset: usize,
}

impl <R: Read + Seek> ByteReader<R> {
    /// Tries to create a new `ByteReader` from an underlying reader.
    fn new(mut reader: R) -> io::Result<Self> {
        // Calculate stream len
        let current = reader.seek(SeekFrom::Current(0))?;
        let length = reader.seek(SeekFrom::End(0))? as usize;
        reader.seek(SeekFrom::Start(current))?;
        Ok(Self{ reader, length, offset: 0 })
    }

    /// Sets the current offset for this reader.
    fn set_offset(&mut self, offset: usize) -> io::Result<()> {
        self.reader.seek(SeekFrom::Start(offset as u64))?;
        self.offset = offset;
        Ok(())
    }

    /// Returns the current offset of this reader.
    fn offset(&self) -> usize { self.offset }

    /// Returns the total length of this reader.
    fn total_len(&self) -> usize { self.length }

    /// Returns the remaining length of this reader.
    fn rem_len(&self) -> usize { self.length - self.offset }

    /// Reads in a little-endian 2-byte unsigned integer.
    fn read_le_u16(&mut self) -> io::Result<u16> {
        let mut bs: [u8; 2] = Default::default();
        self.reader.read_exact(&mut bs)?;
        self.offset += 2;
        Ok(u16::from_le_bytes(bs))
    }

    /// Reads in a little-endian 4-byte unsigned integer.
    fn read_le_u32(&mut self) -> io::Result<u32> {
        let mut bs: [u8; 4] = Default::default();
        self.reader.read_exact(&mut bs)?;
        self.offset += 4;
        Ok(u32::from_le_bytes(bs))
    }

    /// Reads in an exact number of bytes into a `Vec`.
    fn read_to_vec(&mut self, len: usize) -> io::Result<Vec<u8>> {
        let mut v = vec![0u8; len];
        self.reader.read_exact(&mut v)?;
        self.offset += len;
        Ok(v)
    }
}

/// A bit-wise reader.
#[derive(Debug)]
struct BitReader<R: Read> {
    reader: R,
    current_byte: u8,
    bit_index: u8,
}

impl <R: Read> BitReader<R> {
    /// Creates a new `BitReader` from the given reader.
    fn new(reader: R) -> Self {
        Self{
            reader,
            current_byte: 0,
            bit_index: 0,
        }
    }

    /// Reads the next bit from the stream. Either 1 or 0.
    fn read_bit(&mut self) -> io::Result<u8> {
        if self.bit_index == 0 {
            // Read next byte
            self.bit_index = 0;
            let mut bs: [u8; 1] = [0];
            self.reader.read_exact(&mut bs)?;
            self.current_byte = bs[0];
        }
        // Get bit
        let bit = (self.current_byte >> self.bit_index) & 0b1;
        self.bit_index = (self.bit_index + 1) % 8;
        Ok(bit)
    }

    /// Skips to the start of next byte. If already on a byte-boundlary, this is
    /// a no-op.
    fn skip_to_byte(&mut self) {
        self.bit_index = 0;
    }
}

/// The kinds of signature a zip structure can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signature {
    /// No signature.
    None,
    /// Required signature.
    Required(u32),
    /// A signature that's may be present.
    Optional(u32),
}

/// A trait for every zip structure for a common parsing interface.
trait Parse: Sized {
    /// The type of signature this structure has.
    const SIGNATURE: Signature;
    /// The least number of bytes this structure needs to fit (fixed-size
    /// bytes), not counting the signature.
    const FIX_LEN: usize;

    /// Tries to parse this structure from the given bytes. The position of the
    /// reader will be reset, if the parse wasn't successful. On success returns
    /// the parsed structure and the overall consumed bytes.
    ///
    /// The user shouldn't implement this, implement `parse_data` instead.
    fn parse<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)> {
        let offset = r.offset();
        let result = Self::parse_noreset(r);
        if result.is_err() {
            r.set_offset(offset)?;
        }
        result
    }

    /// Tries to parse this structure from the given bytes. The position of the
    /// reader will be unspecified, if the parse wasn't successful. On success
    /// returns the parsed structure and the overall consumed bytes.
    ///
    /// The user shouldn't implement this, implement `parse_data` instead.
    fn parse_noreset<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)> {
        use io::Error;
        use io::ErrorKind::{UnexpectedEof, InvalidData};

        // Check if fix length is in range
        if r.rem_len() < Self::FIX_LEN {
            return Err(Error::new(UnexpectedEof, "Not enough bytes!"));
        }

        match Self::SIGNATURE {
            Signature::None => {
                let (result, consumed) = Self::parse_data(r)?;
                Ok((result, Self::FIX_LEN + consumed))
            },
            Signature::Required(signature) => {
                // Check if signarute and fix length are in range
                if r.rem_len() < Self::FIX_LEN + 4 {
                    return Err(Error::new(UnexpectedEof, "Not enough bytes!"));
                }
                // Check signature
                if r.read_le_u32()? != signature {
                    return Err(Error::new(InvalidData, "Wrong signature!"));
                }
                let (result, consumed) = Self::parse_data(r)?;
                Ok((result, Self::FIX_LEN + consumed + 4))
            },
            Signature::Optional(signature) => {
                // Check if signarute and fix length are in range
                if r.rem_len() >= Self::FIX_LEN + 4 {
                    let offset = r.offset();
                    // Check signature
                    if r.read_le_u32()? == signature {
                        let (result, consumed) = Self::parse_data(r)?;
                        return Ok((result, Self::FIX_LEN + consumed + 4));
                    }
                    // No match, reset
                    r.set_offset(offset)?;
                }
                let (result, consumed) = Self::parse_data(r)?;
                Ok((result, Self::FIX_LEN + consumed))
            }
        }
    }

    /// Tries to parse this structure from the given bytes. The function does
    /// not do bound checks for `FIX_LEN` or the signature. On success returns
    /// the parsed structure and the **non-fix length, extra consumed bytes**.
    fn parse_data<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)>;
}

/// The primary structure we locate for an archive.
/// Specification 4.3.16.
#[repr(C)]
#[derive(Debug)]
struct EndOfCentralDirectoryRecord {
    disk_number           : u16    ,
    central_dir_start_disk: u16    ,
    entries_on_this_disk  : u16    ,
    entries_in_central_dir: u16    ,
    central_dir_size      : u32    ,
    central_dir_offset    : u32    ,
    comment               : Vec<u8>,
}

impl Parse for EndOfCentralDirectoryRecord {
    const SIGNATURE: Signature = Signature::Required(0x06054b50);
    const FIX_LEN: usize = 18;

    fn parse_data<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)> {
        let disk_number            = r.read_le_u16()?;
        let central_dir_start_disk = r.read_le_u16()?;
        let entries_on_this_disk   = r.read_le_u16()?;
        let entries_in_central_dir = r.read_le_u16()?;
        let central_dir_size       = r.read_le_u32()?;
        let central_dir_offset     = r.read_le_u32()?;
        let comment_len            = r.read_le_u16()? as usize;
        // Now the variable-sized comment
        if r.rem_len() < comment_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        let comment = r.read_to_vec(comment_len)?;
        let result = Self{
            disk_number,
            central_dir_start_disk,
            entries_on_this_disk,
            entries_in_central_dir,
            central_dir_size,
            central_dir_offset,
            comment,
        };
        Ok((result, comment_len))
    }
}

impl EndOfCentralDirectoryRecord {
    /// Tries to find the `EndOfCentralDirectoryRecord`. On success returns it
    /// with it's starting offset. The position of the reader will be at the
    /// end of the `EndOfCentralDirectoryRecord` structure.
    fn find<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)> {
        // Go backwards
        let mut offset = r.total_len() - Self::FIX_LEN;
        loop {
            r.set_offset(offset)?;
            if let Ok((r, _)) = EndOfCentralDirectoryRecord::parse_noreset(r) {
                return Ok((r, offset));
            }
            if offset == 0 {
                return Err(io::Error::new(io::ErrorKind::NotFound,
                    "Could not find end of central directory rectord!"));
            }
            offset -= 1;
        }
    }
}

/// The entry in the central directory, that represents a file present in the
/// archive.
/// Specification 4.3.12.
#[repr(C)]
#[derive(Debug)]
struct FileHeader {
    version_made         : u16                     ,
    version_needed       : u16                     ,
    flags                : u16                     ,
    compression          : u16                     ,
    mod_time             : u16                     ,
    mod_date             : u16                     ,
    crc32                : u32                     ,
    compressed_size      : usize                   ,
    uncompressed_size    : usize                   ,
    disk_number          : u16                     ,
    internal_file_attribs: u16                     ,
    external_file_attribs: u32                     ,
    local_header_offset  : u32                     ,
    file_name            : String                  ,
    extra                : Vec<ExtensibleDataField>,
    file_comment         : String                  ,
}

impl Parse for FileHeader {
    const SIGNATURE: Signature = Signature::Required(0x02014b50);
    const FIX_LEN: usize = 42;

    fn parse_data<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)> {
        let version_made          = r.read_le_u16()?;
        let version_needed        = r.read_le_u16()?;
        let flags                 = r.read_le_u16()?;
        let compression           = r.read_le_u16()?;
        let mod_time              = r.read_le_u16()?;
        let mod_date              = r.read_le_u16()?;
        let crc32                 = r.read_le_u32()?;
        let compressed_size       = r.read_le_u32()? as usize;
        let uncompressed_size     = r.read_le_u32()? as usize;
        let file_name_len         = r.read_le_u16()? as usize;
        let extra_len             = r.read_le_u16()? as usize;
        let file_comment_len      = r.read_le_u16()? as usize;
        let disk_number           = r.read_le_u16()?;
        let internal_file_attribs = r.read_le_u16()?;
        let external_file_attribs = r.read_le_u32()?;
        let local_header_offset   = r.read_le_u32()?;
        // Check for space for variable-length
        if r.rem_len() < file_name_len + extra_len + file_comment_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        // Extract
        let is_utf8 = (flags & (1 << 11)) != 0;
        let string_decode = if is_utf8 { decode_utf8 } else { decode_cp437 };
        let file_name = string_decode(&r.read_to_vec(file_name_len)?);
        let (extra, _ec) = ExtensibleDataField::parse_vec(r, extra_len)?;
        let file_comment = string_decode(&r.read_to_vec(file_comment_len)?);
        // All good
        let result = Self{
            version_made,
            version_needed,
            flags,
            compression,
            mod_time,
            mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            disk_number,
            internal_file_attribs,
            external_file_attribs,
            local_header_offset,
            file_name,
            extra,
            file_comment,
        };
        Ok((result, file_name_len + extra_len + file_comment_len))
    }
}

impl FileHeader {
    /// Returns `true`, if the given flag is set.
    fn is_flag(&self, index: usize) -> bool {
        (self.flags & (1 << index)) != 0
    }

    /// Returns `true`, if this header represents a directory.
    fn is_dir(&self) -> bool {
        let lastc = self.file_name.chars().last();
        lastc == Some('/') || lastc == Some('\\')
    }

    /// Returns `true`, if this header represents a file.
    fn is_file(&self) -> bool {
        !self.is_dir()
    }
}

/// Extensible data fields.
/// Specification 4.5.1.
#[repr(C)]
#[derive(Debug)]
struct ExtensibleDataField {
    id  : u16    ,
    data: Vec<u8>,
}

impl Parse for ExtensibleDataField {
    const FIX_LEN: usize = 4;
    const SIGNATURE: Signature = Signature::None;

    fn parse_data<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)> {
        let id       = r.read_le_u16()?;
        let data_len = r.read_le_u16()? as usize;
        // Check if enough for variable-sized
        if r.rem_len() < data_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        // Extract
        let data = r.read_to_vec(data_len)?;
        // All good
        let result = Self{ id, data };
        Ok((result, data_len))
    }
}

impl ExtensibleDataField {
    /// Parses a `Vec<ExtensibleDataField>` as long as it succeeds and is in
    /// range. Returns the read in structures and the number of bytes read.
    /// The function is guaranteed to consume exactly `len` number of bytes on
    /// success, even if it's not an exact fit for the elements.
    fn parse_vec<R: Read + Seek>(r: &mut ByteReader<R>, len: usize) -> io::Result<(Vec<Self>, usize)> {
        let init_offset = r.offset();
        let mut result = Vec::new();
        let mut consumed = 0;
        while consumed < len {
            if let Ok((e, ec)) = Self::parse_noreset(r) {
                consumed += ec;
                if consumed > len {
                    break;
                }
                result.push(e);
            }
            else {
                break;
            }
        }
        // Set to exact amount
        r.set_offset(init_offset + len)?;
        Ok((result, len))
    }
}

/// Local replicas of the directory entries above the actual compressed data.
/// Specification 4.3.7.
#[repr(C)]
#[derive(Debug)]
struct LocalFileHeader {
    version_needed       : u16                     ,
    flags                : u16                     ,
    compression          : u16                     ,
    mod_time             : u16                     ,
    mod_date             : u16                     ,
    crc32                : u32                     ,
    compressed_size      : usize                   ,
    uncompressed_size    : usize                   ,
    file_name            : String                  ,
    extra                : Vec<ExtensibleDataField>,
}

impl Parse for LocalFileHeader {
    const FIX_LEN: usize = 26;
    const SIGNATURE: Signature = Signature::Required(0x04034b50);

    fn parse_data<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<(Self, usize)> {
        let version_needed    = r.read_le_u16()?;
        let flags             = r.read_le_u16()?;
        let compression       = r.read_le_u16()?;
        let mod_time          = r.read_le_u16()?;
        let mod_date          = r.read_le_u16()?;
        let crc32             = r.read_le_u32()?;
        let compressed_size   = r.read_le_u32()? as usize;
        let uncompressed_size = r.read_le_u32()? as usize;
        let file_name_len     = r.read_le_u16()? as usize;
        let extra_len         = r.read_le_u16()? as usize;
        // Check if enough for variable
        if r.rem_len() < file_name_len + extra_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        // Enough, read
        let is_utf8 = (flags & (1 << 11)) != 0;
        let string_decode = if is_utf8 { decode_utf8 } else { decode_cp437 };
        let file_name = string_decode(&r.read_to_vec(file_name_len)?);
        let (extra, _ec) = ExtensibleDataField::parse_vec(r, extra_len)?;
        // All good
        let result = Self{
            version_needed,
            flags,
            compression,
            mod_time,
            mod_date,
            crc32,
            compressed_size,
            uncompressed_size,
            file_name,
            extra,
        };
        Ok((result, file_name_len + extra_len))
    }
}

/// Parses a Zip archive's central directory into `FileHeader` records.
fn parse_central_directory<R: Read + Seek>(r: &mut ByteReader<R>) -> io::Result<Vec<FileHeader>> {
    // First we have to find the end of the central directory
    let (end_of_directory, _eod_start) = EndOfCentralDirectoryRecord::find(r)?;
    // TODO: Find out if Zip64
    // Parse central directory entries
    let mut entries = Vec::new();
    r.set_offset(end_of_directory.central_dir_offset as usize)?;
    for _ in 0..end_of_directory.entries_in_central_dir {
        let (header, _) = FileHeader::parse_noreset(r)?;
        entries.push(header);
    }
    Ok(entries)
}

/// Represents a zipped archive.
pub struct ZipArchive<R: Read + Seek> {
    reader: ByteReader<R>,
    entries: Vec<FileHeader>,
}

impl <R: Read + Seek> ZipArchive<R> {
    /// Tries to parse a `ZipArchive`'s central directory from the given reader.
    pub fn parse(reader: R) -> io::Result<Self> {
        let mut reader = ByteReader::new(reader)?;
        let entries = parse_central_directory(&mut reader)?;
        Ok(Self{ reader, entries })
    }
}

/// The maximum code-length in bits allowed by DEFLATE.
const DEFLATE_MAX_BITS: usize = 15;

/// A type for implementing the DEFLATE decompression algorithm.
#[derive(Debug)]
struct Deflate<R: Read> {
    reader: BitReader<io::Take<R>>,
    //dict: HashMap<u16, u8>,
}

impl <R: Read> Deflate<R> {
    /// Creates a new `Deflate` structure from the given reader reference and
    /// length limit.
    fn new(reader: R, length: usize) -> Self {
        Self{
            reader: BitReader::new(reader.take(length as u64)),
        }
    }
}

impl <R: Read> Read for Deflate<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unimplemented!()
    }
}

/// Generates the canonical Huffman code-value dictionary from the given
/// code-lengths. All codes are prefixed with a 1-bit to be able to
/// differentiate them without storing their lengths.
/// Taken from RFC 1951, 3.2.2.
fn generate_huffman(lens: &[usize]) -> io::Result<HashMap<u16, u16>> {
    // Find the max length
    let max_bits = lens.iter().cloned().max().unwrap_or(0);
    if max_bits > DEFLATE_MAX_BITS {
        return Err(io::Error::new(io::ErrorKind::Other, "Invalid code length!"));
    }
    // Count the number of codes for each code length.  Let bl_count[N] be the
    // number of codes of length N, N >= 1.
    let mut bl_count = [0u16; DEFLATE_MAX_BITS + 1];
    for l in lens {
        bl_count[*l] += 1;
    }
    // Find the numerical value of the smallest code for each code length.
    let mut next_code = vec![0u16; DEFLATE_MAX_BITS + 1];
    let mut code = 0u16;
    // Not setting to 0 to make the extra padding bit on the left
    bl_count[0] = 1;
    for bits in 1..=max_bits {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    // Assign numerical values to all codes, using consecutive values for all
    // codes of the same length with the base values determined at step 2. Codes
    // that are never used (which have a bit length of zero) must not be
    // assigned a value.
    let mut result = HashMap::new();
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

/// Decodes an UTF8 String.
fn decode_utf8(bs: &[u8]) -> String {
    String::from_utf8_lossy(bs).into_owned()
}

/// Decodes a cp437 byte-array into an UTF-8 String.
fn decode_cp437(bs: &[u8]) -> String {
    let mut result = String::with_capacity(bs.len());
    for b in bs {
        let ch = match b {
            // ASCII
            0x0..=0x7F => *b as char,
            // Weird stuff
            128 => '\u{00C7}', 129 => '\u{00FC}', 130 => '\u{00E9}', 131 => '\u{00E2}',
            132 => '\u{00E4}', 133 => '\u{00E0}', 134 => '\u{00E5}', 135 => '\u{00E7}',
            136 => '\u{00EA}', 137 => '\u{00EB}', 138 => '\u{00E8}', 139 => '\u{00EF}',
            140 => '\u{00EE}', 141 => '\u{00EC}', 142 => '\u{00C4}', 143 => '\u{00C5}',
            144 => '\u{00C9}', 145 => '\u{00E6}', 146 => '\u{00C6}', 147 => '\u{00F4}',
            148 => '\u{00F6}', 149 => '\u{00F2}', 150 => '\u{00FB}', 151 => '\u{00F9}',
            152 => '\u{00FF}', 153 => '\u{00D6}', 154 => '\u{00DC}', 155 => '\u{00A2}',
            156 => '\u{00A3}', 157 => '\u{00A5}', 158 => '\u{20A7}', 159 => '\u{0192}',
            160 => '\u{00E1}', 161 => '\u{00ED}', 162 => '\u{00F3}', 163 => '\u{00FA}',
            164 => '\u{00F1}', 165 => '\u{00D1}', 166 => '\u{00AA}', 167 => '\u{00BA}',
            168 => '\u{00BF}', 169 => '\u{2310}', 170 => '\u{00AC}', 171 => '\u{00BD}',
            172 => '\u{00BC}', 173 => '\u{00A1}', 174 => '\u{00AB}', 175 => '\u{00BB}',
            176 => '\u{2591}', 177 => '\u{2592}', 178 => '\u{2593}', 179 => '\u{2502}',
            180 => '\u{2524}', 181 => '\u{2561}', 182 => '\u{2562}', 183 => '\u{2556}',
            184 => '\u{2555}', 185 => '\u{2563}', 186 => '\u{2551}', 187 => '\u{2557}',
            188 => '\u{255D}', 189 => '\u{255C}', 190 => '\u{255B}', 191 => '\u{2510}',
            192 => '\u{2514}', 193 => '\u{2534}', 194 => '\u{252C}', 195 => '\u{251C}',
            196 => '\u{2500}', 197 => '\u{253C}', 198 => '\u{255E}', 199 => '\u{255F}',
            200 => '\u{255A}', 201 => '\u{2554}', 202 => '\u{2569}', 203 => '\u{2566}',
            204 => '\u{2560}', 205 => '\u{2550}', 206 => '\u{256C}', 207 => '\u{2567}',
            208 => '\u{2568}', 209 => '\u{2564}', 210 => '\u{2565}', 211 => '\u{2559}',
            212 => '\u{2558}', 213 => '\u{2552}', 214 => '\u{2553}', 215 => '\u{256B}',
            216 => '\u{256A}', 217 => '\u{2518}', 218 => '\u{250C}', 219 => '\u{2588}',
            220 => '\u{2584}', 221 => '\u{258C}', 222 => '\u{2590}', 223 => '\u{2580}',
            224 => '\u{03B1}', 225 => '\u{00DF}', 226 => '\u{0393}', 227 => '\u{03C0}',
            228 => '\u{03A3}', 229 => '\u{03C3}', 230 => '\u{00B5}', 231 => '\u{03C4}',
            232 => '\u{03A6}', 233 => '\u{0398}', 234 => '\u{03A9}', 235 => '\u{03B4}',
            236 => '\u{221E}', 237 => '\u{03C6}', 238 => '\u{03B5}', 239 => '\u{2229}',
            240 => '\u{2261}', 241 => '\u{00B1}', 242 => '\u{2265}', 243 => '\u{2264}',
            244 => '\u{2320}', 245 => '\u{2321}', 246 => '\u{00F7}', 247 => '\u{2248}',
            248 => '\u{00B0}', 249 => '\u{2219}', 250 => '\u{00B7}', 251 => '\u{221A}',
            252 => '\u{207F}', 253 => '\u{00B2}', 254 => '\u{25A0}', 255 => '\u{00A0}',
        };
        result.push(ch);
    }
    result
}

fn test_huffman(lens: &[usize]) {
    let dict = generate_huffman(lens).unwrap();
    println!("Lengths: {:?}", lens);
    for (c, v) in &dict {
        println!("{:b} -> {}", c, v);
    }
}

pub fn test(path: impl AsRef<Path>) {
    /*let f = fs::File::open(path).unwrap();
    let mut fr = ByteReader::new(f).expect("msg: &str");
    let entries = parse_central_directory(&mut fr).expect("msg: &str");
    for e in &entries {
        println!("{}", e.file_name);
    }*/

    test_huffman(&[1, 0, 3, 2, 3]);
    test_huffman(&[2, 2, 1, 0, 0, 0]);
}

#[cfg(test)]
mod tests {
    use super::*;
}

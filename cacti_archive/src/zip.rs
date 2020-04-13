//! Zip archive handling.
// TODO: doc

use std::io::{Read, Seek, SeekFrom};
use std::io;
use std::time::{SystemTime, Duration};
use std::convert::{TryFrom, TryInto};
use crate::deflate::Inflate;

/// A structure for calculating CRC32.
struct Crc32(u32);

impl Crc32 {
    /// The magic number used in CRC.
    const MAGIC: u32 = 0xdebb20e3;

    /// Creates a new `Crc32` with a default value.
    fn new() -> Self { Self(0xffffffff) }

    /// Returns the result of the `Crc32`.
    fn finalize(self) -> u32 { !self.0 }

    /// Adds a byte to the `Crc32`.
    fn push(&mut self, byte: u8) {
        self.0 ^= byte as u32;
        for _ in 0..8 {
            let mask = !(self.0 & 1).wrapping_sub(1);
            self.0 = (self.0 >> 1) ^ (Self::MAGIC & mask);
        }
    }
}

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

    /// Returns a reference to the underlying reader.
    fn reader_ref(&mut self) -> &mut R { &mut self.reader }

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

/// The enumeration of supported compression algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compression {
    NoCompression,
    Deflate,
}

impl TryFrom<u16> for Compression {
    type Error = io::Error;

    fn try_from(n: u16) -> io::Result<Self> {
        match n {
            0 => Ok(Self::NoCompression),
            8 => Ok(Self::Deflate),
            _ => Err(io::Error::new(io::ErrorKind::Other, "Unsupported compression!")),
        }
    }
}

impl Compression {
    /// Creates a decompressor for this compression algorithm with the given
    /// reader and given compressed length.
    fn create_decompressor<R: Read>(&self, reader: R, compressed_size: usize) -> ZipFileDecompressor<R> {
        let reader = reader.take(compressed_size as u64);
        match self {
            Self::NoCompression => ZipFileDecompressor::NoCompression(reader)        ,
            Self::Deflate       => ZipFileDecompressor::Deflate(Inflate::new(reader)),
        }
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
#[derive(Debug)]
pub struct ZipArchive<R: Read + Seek> {
    reader : ByteReader<R>  ,
    entries: Vec<FileHeader>,
}

impl <R: Read + Seek> ZipArchive<R> {
    /// Tries to parse a `ZipArchive`'s central directory from the given reader.
    pub fn parse(reader: R) -> io::Result<Self> {
        let mut reader = ByteReader::new(reader)?;
        let entries = parse_central_directory(&mut reader)?;
        Ok(Self{ reader, entries })
    }

    /// Returns the number of `ZipFile` entries this archive holds.
    pub fn entry_count(&self) -> usize { self.entries.len() }

    /// Returns the `ZipFile` descriptor for the given entry index.
    pub fn entry_at_index<'a>(&'a mut self, index: usize) -> io::Result<ZipFile<'a, R>> {
        ZipFile::new(&mut self.reader, &self.entries[index])
    }
}

/// Represents a single file or directory inside a `ZipArchive`.
#[derive(Debug)]
pub struct ZipFile<'a, R: Read + Seek> {
    reader           : &'a mut R  ,
    name             : &'a str    ,
    is_encrypted     : bool       ,
    is_file          : bool       ,
    last_modified    : SystemTime ,
    compression      : Compression,
    data_offset      : usize      ,
    compressed_size  : usize      ,
    uncompressed_size: usize      ,
    crc32            : u32        ,
}

// TODO: Fix CRC32 checks

impl <'a, R: Read + Seek> ZipFile<'a, R> {
    /// Creates the `ZipFile` from the given reader and `FileHeader`.
    fn new(reader: &'a mut ByteReader<R>, header: &'a FileHeader) -> io::Result<Self> {
        // File name
        let mut name = header.file_name.as_str();
        if header.is_dir() {
            // We remove the '/'
            name = &name[..(name.len() - 1)];
        }
        // Data offset
        reader.set_offset(header.local_header_offset as usize)?;
        let _local_header = LocalFileHeader::parse_noreset(reader)?;
        let data_offset = reader.offset();
        // Done
        Ok(Self {
            reader: reader.reader_ref(),
            name,
            is_encrypted: header.is_flag(0),
            is_file: header.is_file(),
            last_modified: decode_ms_dos_datetime(header.mod_date, header.mod_time),
            compression: header.compression.try_into()?,
            data_offset,
            compressed_size: header.compressed_size,
            uncompressed_size: header.uncompressed_size,
            crc32: header.crc32,
        })
    }

    /// Returns the full path and name of this file or directory.
    pub fn name(&self) -> &str { &self.name }

    /// Returns `true`, if this entry is a file.
    pub fn is_file(&self) -> bool { self.is_file }
    /// Returns `true`, if this entry is a directory.
    pub fn is_dir(&self) -> bool { !self.is_file }

    /// Returns the stored modification time.
    pub fn modification_time(&self) -> SystemTime { self.last_modified }

    /// Returns the byte-size of the file this represents, when compressed.
    pub fn compressed_size(&self) -> usize { self.compressed_size }
    /// Returns the byte-size of the file this represents, when uncompressed.
    /// This can be used to pre-allocate a buffer for decompression.
    pub fn uncompressed_size(&self) -> usize { self.uncompressed_size }

    /// Returns the decompressor for this file. Use `uncompressed_size` as a
    /// length to  pre-allocate a buffer for the optimal allocation size.
    pub fn decompressor(&'a mut self) -> io::Result<impl Read + 'a> {
        if self.is_encrypted {
            return Err(io::Error::new(io::ErrorKind::Other, "Encryption is not supported!"));
        }
        self.reader.seek(io::SeekFrom::Start(self.data_offset as u64))?;
        Ok(self.compression.create_decompressor(&mut self.reader, self.compressed_size))
    }

    /// Checks integrity using the stored CRC32 value. Returns `true`, if the
    /// check was valid.
    pub fn check_crc32(&mut self) -> io::Result<bool> {
        const BUFFER_SIZE: usize = 32;

        let mut buffer = [0u8; BUFFER_SIZE];
        self.reader.seek(io::SeekFrom::Start(self.data_offset as u64))?;

        let mut crc = Crc32::new();
        let mut remaining = self.compressed_size;
        while remaining > 0 {
            let can_read = std::cmp::min(remaining, BUFFER_SIZE);
            self.reader.read_exact(&mut buffer[..can_read])?;

            for i in 0..can_read {
                crc.push(buffer[i]);
            }

            remaining -= can_read;
        }

        Ok(crc.finalize() == self.crc32)
    }
}

/// Represents a `ZipFile` decompressor.
#[derive(Debug)]
enum ZipFileDecompressor<R: Read> {
    NoCompression(io::Take<R>),
    Deflate(Inflate<io::Take<R>>),
}

impl <R: Read> io::Read for ZipFileDecompressor<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::NoCompression(r) => r.read(buf),
            Self::Deflate(r)       => r.read(buf),
        }
    }
}

/// Translates the MS-DOS date-time format to `SystemTime`.
fn decode_ms_dos_datetime(date: u16, time: u16) -> SystemTime {
    let dos_epoch = SystemTime::UNIX_EPOCH + Duration::from_secs(315532800);
    let date_offs = Duration::from_secs(24 * 60 * 60 * (date as u64));
    let time_offs = Duration::from_secs((time as u64) * 2);
    dos_epoch + date_offs + time_offs
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

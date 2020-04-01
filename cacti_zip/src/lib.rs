
use std::path::Path;
use std::io::Read;
use std::collections::HashMap;
use std::fs;
use std::io;

// Some minimal parser abstraction

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signature {
    None,
    Required(u32),
    Optional(u32),
}

trait Parse: Sized {
    const FIX_LEN  : usize;
    const SIGNATURE: Signature;

    fn parse(b: &[u8]) -> io::Result<(Self, usize)> {
        match Self::SIGNATURE {
            Signature::None => {
                // Check if fix length is in range
                if b.len() < Self::FIX_LEN {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                        "Not enough bytes!"));
                }
                let (result, consumed) = Self::parse_internal(b)?;
                Ok((result, Self::FIX_LEN + consumed))
            },
            Signature::Required(signature) => {
                // Check if signarute and fix length are in range
                if b.len() < Self::FIX_LEN + 4 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                        "Not enough bytes!"));
                }
                // Check signature
                let s = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if s != signature {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Wrong signature!"));
                }
                let (result, consumed) = Self::parse_internal(&b[4..])?;
                Ok((result, Self::FIX_LEN + consumed + 4))
            },
            Signature::Optional(signature) => {
                // Check if signarute and fix length are in range
                let could_have_signature = b.len() >= Self::FIX_LEN + 4;
                unimplemented!();
            }
        }
    }

    fn parse_internal(b: &[u8]) -> io::Result<(Self, usize)>;
}

// end of central directory record /////////////////////////////////////////////

#[derive(Debug)]
struct EndOfCentralDirectoryRecord {
    disk_number                 : u16    ,
    central_dir_start_disk      : u16    ,
    entries_on_this_disk        : u16    ,
    total_entries_in_central_dir: u16    ,
    central_dir_size            : u32    ,
    central_dir_offset          : u32    ,
    comment                     : Vec<u8>,
}

impl EndOfCentralDirectoryRecord {
    /// The minimum offset backwards to search for.
    const MIN_BYTES: usize = 22;

    /// Parses the bytes into a `EndOfCentralDirectoryRecord` structure. If
    /// succeeded, returns the read in structure and the number of bytes read.
    fn parse(b: &[u8]) -> io::Result<(Self, usize)> {
        // At least 22 bytes without variable fields
        if b.len() < Self::MIN_BYTES {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        // Signature must be 0x06054b50
        let signature = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if signature != 0x06054b50 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Wrong signature!"));
        }
        // Read in the rest of the things freely, until variable size
        let disk_number = u16::from_le_bytes([b[4], b[5]]);
        let central_dir_start_disk = u16::from_le_bytes([b[6], b[7]]);
        let entries_on_this_disk = u16::from_le_bytes([b[8], b[9]]);
        let total_entries_in_central_dir = u16::from_le_bytes([b[10], b[11]]);
        let central_dir_size = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        let central_dir_offset = u32::from_le_bytes([b[16], b[17], b[18], b[19]]);
        let comment_len = u16::from_le_bytes([b[20], b[21]]) as usize;
        // Now the variable-sized comment
        let b = &b[22..];
        if b.len() < comment_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        let comment = Vec::from(&b[0..comment_len]);
        // All good
        let consumed = Self::MIN_BYTES + comment_len;
        let result = Self{
            disk_number,
            central_dir_start_disk,
            entries_on_this_disk,
            total_entries_in_central_dir,
            central_dir_size,
            central_dir_offset,
            comment,
        };
        Ok((result, consumed))
    }

    /// Tries to find the `EndOfCentralDirectoryRecord`. On success returns it
    /// with it's starting offset.
    fn find(b: &[u8]) -> io::Result<(Self, usize)> {
        if b.len() < Self::MIN_BYTES {
            // Invalid
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        // Go backwards
        let mut offset = b.len() - EndOfCentralDirectoryRecord::MIN_BYTES;
        loop {
            let sub_b = &b[offset..];
            if let Ok((r, _)) = EndOfCentralDirectoryRecord::parse(sub_b) {
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

// central directory header ////////////////////////////////////////////////////

#[derive(Debug)]
struct FileHeader {
    version_made         : u16            ,
    version_needed       : u16            ,
    flags                : u16            ,
    compression          : u16            ,
    mod_time             : u16            ,
    mod_date             : u16            ,
    crc32                : u32            ,
    compressed_size      : usize          ,
    uncompressed_size    : usize          ,
    disk_number          : u16            ,
    internal_file_attribs: u16            ,
    external_file_attribs: u32            ,
    local_header_offset  : u32            ,
    file_name            : String         ,
    extra                : Vec<ExtraField>,
    file_comment         : String         ,
}

impl FileHeader {
    /// Parses the bytes into a `FileHeader` structure. If succeeded, returns
    /// the read in structure and the number of bytes read.
    fn parse(b: &[u8]) -> io::Result<(Self, usize)> {
        // Make sure to have enough bytes
        if b.len() < 46 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        // Signature must be 0x02014b50
        let signature = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if signature != 0x02014b50 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Wrong signature!"));
        }
        // Read fix size
        let version_made = u16::from_le_bytes([b[4], b[5]]);
        let version_needed = u16::from_le_bytes([b[6], b[7]]);
        let flags = u16::from_le_bytes([b[8], b[9]]);
        let compression = u16::from_le_bytes([b[10], b[11]]);
        let mod_time = u16::from_le_bytes([b[12], b[13]]);
        let mod_date = u16::from_le_bytes([b[14], b[15]]);
        let crc32 = u32::from_le_bytes([b[16], b[17], b[18], b[19]]);
        let compressed_size = u32::from_le_bytes([b[20], b[21], b[22], b[23]]) as usize;
        let uncompressed_size = u32::from_le_bytes([b[24], b[25], b[26], b[27]]) as usize;
        let file_name_len = u16::from_le_bytes([b[28], b[29]]) as usize;
        let extra_len = u16::from_le_bytes([b[30], b[31]]) as usize;
        let file_comment_len = u16::from_le_bytes([b[32], b[33]]) as usize;
        let disk_number = u16::from_le_bytes([b[34], b[35]]);
        let internal_file_attribs = u16::from_le_bytes([b[36], b[37]]);
        let external_file_attribs = u32::from_le_bytes([b[38], b[39], b[40], b[41]]);
        let local_header_offset = u32::from_le_bytes([b[42], b[43], b[44], b[45]]);
        // Enough space for variable
        let b = &b[46..];
        if b.len() < file_name_len + extra_len + file_comment_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes!"));
        }
        // Extract
        let is_utf8 = (flags & (1 << 11)) != 0;
        let string_decode = if is_utf8 { decode_utf8 } else { decode_cp437 };
        let file_name = string_decode(&b[0..file_name_len]);
        let b = &b[file_name_len..];
        let extra = &b[0..extra_len];
        let (extra, _ec) = ExtraField::parse_vec(extra);
        // assert_eq!(_ec, extra_len);
        let b = &b[extra_len..];
        let file_comment = string_decode(&b[0..file_comment_len]);
        // All good
        let consumed = 46 + file_name_len + extra_len + file_comment_len;
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
        Ok((result, consumed))
    }

    /// Returns true, if the given flag is set.
    fn is_flag(&self, index: usize) -> bool {
        (self.flags & (1 << index)) != 0
    }

    /// Returns true, if this entry is a directory.
    fn is_dir(&self) -> bool {
        let lastc = self.file_name.chars().last();
        lastc == Some('/') || lastc == Some('\\')
    }

    /// Returns true, if this entry is a file.
    fn is_file(&self) -> bool {
        !self.is_dir()
    }
}

// extra field /////////////////////////////////////////////////////////////////

#[derive(Debug)]
struct ExtraField {
    id  : u16    ,
    data: Vec<u8>,
}

impl ExtraField {
    /// Parses the bytes into an `ExtraField` structure. If succeeded, returns
    /// the read in structure and the number of bytes read.
    fn parse(b: &[u8]) -> io::Result<(Self, usize)> {
        // Check for at least header bytes
        if b.len() < 4 {
            return None;
        }
        // Read in fix
        let id = u16::from_le_bytes([b[0], b[1]]);
        let data_len = u16::from_le_bytes([b[2], b[3]]) as usize;
        // Check if enough for variable
        let b = &b[4..];
        if b.len() < data_len {
            return None;
        }
        // Read in
        let data = Vec::from(&b[0..data_len]);
        // All good
        let consumed = 4 + data_len;
        let result = Self{ id, data };
        Some((result, consumed))
    }

    /// Parses a `Vec<ExtraField>` as long as it succeeds. Returns the read in
    /// structures and the number of bytes read.
    fn parse_vec(mut b: &[u8]) -> (Vec<Self>, usize) {
        let mut result = Vec::new();
        let mut consumed = 0;
        while let Some((e, offs)) = Self::parse(b) {
            result.push(e);
            consumed += offs;
            b = &b[offs..];
        }
        (result, consumed)
    }
}

// local file header ///////////////////////////////////////////////////////////

#[derive(Debug)]
struct LocalFileHeader {
    version_needed       : u16            ,
    flags                : u16            ,
    compression          : u16            ,
    mod_time             : u16            ,
    mod_date             : u16            ,
    crc32                : u32            ,
    compressed_size      : usize          ,
    uncompressed_size    : usize          ,
    file_name            : String         ,
    extra                : Vec<ExtraField>,
}

impl LocalFileHeader {
    /// Parses the bytes into a `LocalFileHeader` structure. If succeeded,
    /// returns the read in structure and the number of bytes read.
    fn parse(b: &[u8]) -> Option<(Self, usize)> {
        // Make sure to have enough bytes
        if b.len() < 30 {
            return None;
        }
        // Signature must be 0x02014b50
        let signature = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if signature != 0x04034b50 {
            return None;
        }
        // Read fixed size
        let version_needed = u16::from_le_bytes([b[4], b[5]]);
        let flags = u16::from_le_bytes([b[6], b[7]]);
        let compression = u16::from_le_bytes([b[8], b[9]]);
        let mod_time = u16::from_le_bytes([b[10], b[11]]);
        let mod_date = u16::from_le_bytes([b[12], b[13]]);
        let crc32 = u32::from_le_bytes([b[14], b[15], b[16], b[17]]);
        let compressed_size = u32::from_le_bytes([b[18], b[19], b[20], b[21]]) as usize;
        let uncompressed_size = u32::from_le_bytes([b[22], b[23], b[24], b[25]]) as usize;
        let file_name_len = u16::from_le_bytes([b[26], b[27]]) as usize;
        let extra_len = u16::from_le_bytes([b[28], b[29]]) as usize;
        // Check if enough for variable
        let b = &b[30..];
        if b.len() < file_name_len + extra_len {
            return None;
        }
        // Enough, read
        let is_utf8 = (flags & (1 << 11)) != 0;
        let file_name = &b[0..file_name_len];
        let file_name = if is_utf8 { decode_utf8(file_name) } else { decode_cp437(file_name) };
        let b = &b[file_name_len..];
        let extra = &b[0..extra_len];
        let (extra, _ec) = ExtraField::parse_vec(extra);
        // assert_eq!(_ec, extra_len);
        // All good
        let consumed = 30 + file_name_len + extra_len;
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
        Some((result, consumed))
    }
}

// Parsing an archive //////////////////////////////////////////////////////////

fn parse_archive(b: &[u8]) {
    // First find the 'end of central directory record'
    let end_record = EndOfCentralDirectoryRecord::find(b);
    if end_record.is_none() {
        println!("No end record");
        return;
    }
    let (end_record, _) = end_record.unwrap();
    // TODO: Zip64
    // let is_zip64 = false;
    // Parse central directory entries
    let mut dir_entries = Vec::new();
    {
        let mut offset = end_record.central_dir_offset as usize;
        for _ in 0..end_record.total_entries_in_central_dir {
            let header = FileHeader::parse(&b[offset..]);
            if header.is_none() {
                println!("Corrupt directory entry");
                return;
            }
            let (header, offs) = header.unwrap();
            offset += offs;
            dir_entries.push(header);
        }
    }

    // For now we just match local headers
    for e in &dir_entries {
        if e.is_flag(0) {
            // Encrypted
            unimplemented!();
        }

        let hoffs = e.local_header_offset as usize;
        let sub = &b[hoffs..];
        if let Some((_loc, loc_size)) = LocalFileHeader::parse(sub) {
            println!("Trying to decompress {}:", e.file_name);
            let data_offset = hoffs + loc_size;
            let data_size = e.compressed_size;
            if data_offset + data_size > b.len() {
                println!("VERI BIG OOF");
                break;
            }
            let data = &b[data_offset..(data_offset + data_size)];
            // Deflate
            if e.compression == 8 {
                let result = deflate(data);
                println!("Deflated: {:?}", result);
            }
            else {
                println!("Unknown compression: {}", e.compression);
            }
        }
        else {
            println!("BIG OOF");
        }
    }
}

// Deflate /////////////////////////////////////////////////////////////////////

// Bit-stream reader helper ////////////

struct BitReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl <'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self{ bytes, position: 0 }
    }

    fn read_to_u8(&mut self, count: usize) -> Option<u8> {
        const BITMASKS: [u8; 9] = [
            0b00000000, 0b00000001, 0b00000011,
            0b00000111, 0b00001111, 0b00011111,
            0b00111111, 0b01111111, 0b11111111,
        ];
        let result;
        let byte = self.position / 8;
        let bit = self.position % 8;
        // Bounds-check
        if byte >= self.bytes.len() {
            return None;
        }
        if bit + count <= 8 {
            // We fit into this bit
            result = (self.bytes[byte] >> bit) & BITMASKS[count];
        }
        else {
            // From 2 parts, bounds-check
            if byte + 1 >= self.bytes.len() {
                return None;
            }
            // Concatenate from 2 bytes
            let rem = 8 - bit;
            let next_rem = count - rem;
            result =   (self.bytes[byte] >> rem)
                     | ((self.bytes[byte + 1] >> (8 - next_rem)) & BITMASKS[next_rem]);
        }
        self.position += count;
        Some(result)
    }

    fn align_to_byte(&mut self) {
        let offs = 8 - self.position % 8;
        if offs == 8 {
            return;
        }
        self.position += offs;
    }

    fn read_aligned_u16(&mut self) -> Option<u16> {
        assert_eq!(self.position % 8, 0);
        let byte = self.position / 8;
        // Bounds check
        if byte + 1 >= self.bytes.len() {
            return None;
        }
        self.position += 16;
        Some(u16::from_le_bytes([self.bytes[byte], self.bytes[byte + 1]]))
    }

    fn read_aligned(&mut self, buffer: &mut [u8]) -> bool {
        assert_eq!(self.position % 8, 0);
        let byte = self.position / 8;
        // Bounds check
        if byte + buffer.len() > self.bytes.len() {
            return false;
        }
        self.position += 8 * buffer.len();
        buffer.clone_from_slice(&self.bytes[byte..(byte + buffer.len())]);
        true
    }
}

////////////////////////////////////////

fn deflate(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut reader = BitReader::new(bytes);
    loop {
        // Block header
        let is_last = reader.read_to_u8(1);
        let block_type = reader.read_to_u8(2);
        if is_last.is_none() || block_type.is_none() {
            // Error
            return None;
        }
        let is_last = is_last.unwrap() != 0;
        let block_type = block_type.unwrap();

        if block_type == 0b11 {
            // Reserved, error
            println!("Reserved header error");
            return None;
        }

        if block_type == 0b00 {
            reader.align_to_byte();
            // Read LEN and NLEN
            let len = reader.read_aligned_u16();
            let nlen = reader.read_aligned_u16();
            if len.is_none() || nlen.is_none() {
                return None;
            }
            let len = len.unwrap();
            let nlen = nlen.unwrap();
            // Do we need to check this?
            if !len != nlen {
                return None;
            }
            // Reserve for block
            let len = len as usize;
            let output_offs = output.len();
            output.resize(output_offs + len, 0u8);
            // Read
            if !reader.read_aligned(&mut output[output_offs..(output_offs + len)]) {
                return None;
            }
        }
        else {
            let mut dict = HashMap::new();
            let mut min_len = 16;
            if block_type == 0b10 {
                // TODO: Read in dynamic Huffman code
                unimplemented!("Dynamic huffman loading");
            }
            else {
                // Default Huffman
                for i in 0..=143   { dict.insert((8, 0b00110000  + i), i); };
                for i in 144..=255 { dict.insert((9, 0b110010000 + i), i); };
                for i in 256..=279 { dict.insert((7, 0b0000000   + i), i); };
                for i in 280..=287 { dict.insert((8, 0b11000000  + i), i); };
                min_len = 7;
            }
            // Get the next value decoded
            // TODO: Probably not u8, since it could get longer
            // Gotta check
            let code = reader.read_to_u8(min_len);
            if code.is_none() {
                return None;
            }
            let mut code =
            unimplemented!("Huffman");
        }

        if is_last {
            break;
        }
    }
    Some(output)
}

fn generate_huffman_codes(lens: &[usize]) -> Vec<Option<u64>> {
    // Find the max length
    let max_bits = lens.iter().cloned().max().unwrap_or(0);
    // Step 1
    let mut bl_count = vec![0u64; max_bits + 1];
    for l in lens {
        bl_count[*l] += 1;
    }
    // Step 2
    let mut next_code = vec![0u64; max_bits + 1];
    let mut code = 0u64;
    for bits in 1..=max_bits {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    // Step 3
    let mut result = vec![None; lens.len()];
    for n in 0..lens.len() {
        let len = lens[n];
        if len != 0 {
            let code = next_code[len];
            result[n] = Some(code);
            next_code[len] += 1;
        }
    }
    result
}

////////////////////////////////////////////////////////////////////////////////

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

pub fn test(path: impl AsRef<Path>) {
    let mut f = fs::File::open(path).unwrap();
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer).expect("REE");

    parse_archive(&buffer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader() {
        let bytes = vec![0b11010111, 0b00100010, 0b11011011];
        let mut bs = BitReader::new(&bytes);

        bs.align_to_byte();

        assert_eq!(bs.read_to_u8(3), Some(0b111));
        assert_eq!(bs.read_to_u8(1), Some(0b0));
        assert_eq!(bs.read_to_u8(1), Some(0b1));
        assert_eq!(bs.read_to_u8(5), Some(0b01101));
        assert_eq!(bs.read_to_u8(4), Some(0b0001));

        bs.align_to_byte();

        assert_eq!(bs.read_to_u8(3), Some(0b110));
    }
}

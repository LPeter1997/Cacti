
use std::path::Path;
use std::io::Read;
use std::fs;

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
    fn parse(b: &[u8]) -> Option<(Self, usize)> {
        // At least 22 bytes without variable fields
        if b.len() < Self::MIN_BYTES {
            return None;
        }
        // Signature must be 0x06054b50
        let signature = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if signature != 0x06054b50 {
            return None;
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
            return None;
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
        Some((result, consumed))
    }

    /// Tries to find the `EndOfCentralDirectoryRecord`. On success returns it
    /// with it's starting offset.
    fn find(b: &[u8]) -> Option<(Self, usize)> {
        if b.len() < Self::MIN_BYTES {
            // Invalid
            return None;
        }
        // Go backwards
        let mut offset = b.len() - EndOfCentralDirectoryRecord::MIN_BYTES;
        loop {
            let sub_b = &b[offset..];
            if let Some((r, _)) = EndOfCentralDirectoryRecord::parse(sub_b) {
                return Some((r, offset));
            }
            if offset == 0 {
                return None;
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
    fn parse(b: &[u8]) -> Option<(Self, usize)> {
        // Make sure to have enough bytes
        if b.len() < 46 {
            return None;
        }
        // Signature must be 0x02014b50
        let signature = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if signature != 0x02014b50 {
            return None;
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
            return None;
        }
        // Extract
        let is_utf8 = (flags & (1 << 11)) != 0;
        let string_decode = if is_utf8 { decode_utf8 } else { decode_cp437 };
        let file_name = string_decode(&b[0..file_name_len]);
        let b = &b[file_name_len..];
        let extra = &b[0..extra_len];
        let (extra, _consumed) = ExtraField::parse_vec(extra);
        // assert_eq!(_consumed, extra_len);
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
        Some((result, consumed))
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
    fn parse(b: &[u8]) -> Option<(Self, usize)> {
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

// Parsing an archive //////////////////////////////////////////////////////////

fn parse_archive(b: &[u8]) {
    // First find the 'end of central directory record'
    if let Some((end, offs)) = EndOfCentralDirectoryRecord::find(b) {
        // TODO: Zip64
        let is_zip64 = false;

        let mut current_offs = end.central_dir_offset as usize;
        for _ in 0..end.total_entries_in_central_dir {
            if let Some((header, offs)) = FileHeader::parse(&b[current_offs..]) {
                println!("HEADER: {:?}", header);
                current_offs += offs;
            }
            else {
                println!("Big oof");
            }
        }
    }
    else {
        println!("Could not find end!");
    }
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
    f.read_to_end(&mut buffer);

    parse_archive(&buffer);
}

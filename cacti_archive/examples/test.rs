
use std::fs;
use std::io::Read;
use cacti_archive::*;

fn main() {
    let f = fs::File::open("C:/TMP/testzippy.ZIP").expect("Could not open file!");
    let mut archive = zip::ZipArchive::parse(f).expect("Could not parse zip!");
    let mut file = archive.entry_at_index(3).expect("Could not get entry!");
    let crc = file.check_crc32().expect("Could not check crc32!");
    println!("Fname: {}", file.name());
    println!("CRC32 check: {}", crc);
    let mut dec = file.decompressor().expect("Can't decompress file!");
    let mut content = Vec::new();
    dec.read_to_end(&mut content).expect("Can't decompress 2!");
    for c in content {
        print!("{}", c as char);
    }
}

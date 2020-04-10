
use std::time::SystemTime;
use std::io::{Read, Write};
use std::fs;
use std::env;

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = KILOBYTE * 1024;

const REPEAT: usize = 20;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        println!("Usage: {} <source> <destination>", args[0]);
        return;
    }
    let mut buffer = Vec::with_capacity(20 * MEGABYTE);
    let mut file = fs::File::open(&args[1]).expect("Could not open file!");
    file.read_to_end(&mut buffer).expect("Could not read file!");

    let mut out_buffer = Vec::with_capacity(20 * MEGABYTE);

    let mut results = Vec::new();
    for _ in 0..REPEAT {
        out_buffer.clear();
        let mut infl = cacti_archive::Inflate::new(buffer.as_slice());
        let start = SystemTime::now();
        infl.read_to_end(&mut out_buffer).expect("Could not decompress!");
        let end = SystemTime::now();
        let elapsed = end.duration_since(start).expect("Time went backwards!");
        results.push(elapsed);
    }

    for i in 0..REPEAT {
        println!("{}", results[i].as_millis());
    }
    println!("==========");

    let mut out_file = fs::File::create(&args[2]).expect("Could not open target file!");
    out_file.write_all(&out_buffer).expect("Could not write output!");
}


use std::io::Read;
use std::io::BufRead;
use fs_temp::*;

fn main() -> std::io::Result<()> {
    {
        let f = directory_in("C:/TMP/szavak")?;
        let stdin = std::io::stdin();
        let line1 = stdin.lock().lines().next().unwrap().unwrap();
    }
    Ok(())
}

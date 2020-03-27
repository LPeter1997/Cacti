
use fs_temp::*;

fn main() {
    println!("{:?}", directory());
    println!("{:?}", file(None));
    println!("{:?}", directory_in("C:/TMP"));
    println!("{:?}", file_in("C:/TMP", Some("txt")));
}

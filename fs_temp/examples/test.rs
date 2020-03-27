
use fs_temp::*;

fn main() {
    println!("{:?}", path(None));
    println!("{:?}", path_in("C:/TMP", Some("txt")));
}

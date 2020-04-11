
use cacti_asset::dyn_lib::*;

fn main() {
    let mut lib = Library::load("kernel32").expect("Could not load library!");
    let sym: Symbol<extern "system" fn(u32) -> u32> = lib.load_symbol("GetProcessVersion").expect("Could not load symbol!");
    let v = sym(0);
    println!("Ree: {}", v);
}

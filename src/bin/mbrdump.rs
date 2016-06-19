extern crate libpart;

use libpart::mbr;
use std::env;
use std::fs::File;

fn main() {

    let args = env::args().collect::<Vec<_>>();

    let name = &args[1];

    let mut file = File::open(name).unwrap();

    let mbr = mbr::MBR::load(&mut file).unwrap();
   
    println!("You have {} MBR partition(s)", mbr.partition_count());

    for p in mbr.partitions().iter().enumerate().filter(|p| p.1.is_some()) {
        match p {
            (num, &Some(ref p)) => {
                println!("Partition #{}: Type: {}", num + 1, p.system_id);
            },
            _ => {}
        }
    }
    
}

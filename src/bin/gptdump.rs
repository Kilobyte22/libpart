extern crate libpart;

use libpart::gpt;
use std::env;
use std::fs::File;

fn main() {

    let args = env::args().collect::<Vec<_>>();

    println!("{:?}", args);

    let file_name = &args[1];

    println!("Reading GPT of {}", file_name);

    let mut file = File::open(file_name).unwrap();

    let table = gpt::GPTTable::load(&mut file, &gpt::GPTOptions::default()).unwrap();

    println!("You have {} partition(s)", table.part_count());

    for p in table.partitions().iter().enumerate().filter(|p| p.1.is_some()) {
        match p {
            (id, &Some(ref info)) => println!("Partition #{}: Type {} is called \"{}\"", id + 1, info.part_type, info.name),
            _ => unreachable!("FUCKEM")
        }
    }

}

extern crate libpart;

use std::env;
use std::fs::{File, OpenOptions};
use libpart::gpt;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let input = &args[1];
    let output = &args[2];

    let mut ifile = File::open(input).unwrap();
    let mut ofile = OpenOptions::new().read(true).write(true).create(false).open(output).unwrap();

    let gpt = gpt::GPTTable::load(&mut ifile, &gpt::GPTOptions::default()).unwrap();

    gpt.write(&mut ofile, &gpt::GPTOptions::default()).unwrap();
}

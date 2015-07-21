// Similar to the evtest tool.

extern crate evdev;

use std::io::prelude::*;

fn main() {
    let mut args = std::env::args_os();
    let mut d;
    if args.len() > 1 {
        d = evdev::Device::open(&args.nth(1).unwrap()).unwrap();
    } else {
        let mut devices = evdev::enumerate();
        for (i, d) in devices.iter().enumerate() {
            println!("{}: {:?}", i, d.name());
        }
        print!("Select the device [0-{}]: ", devices.len());
        let _ = std::io::stdout().flush();
        let mut chosen = String::new();
        std::io::stdin().read_line(&mut chosen).unwrap();
        d = devices.swap_remove(chosen.trim().parse::<usize>().unwrap());
    }
    println!("{}", d);
    println!("Events:");
    loop {
        for ev in d.events_no_sync().unwrap() {
            println!("{:?}", ev);
        }
    }
}

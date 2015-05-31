extern crate evdev;

fn main() {
    for d in evdev::enumerate() {
        println!("{:?}", d);
    }
}

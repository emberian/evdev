use tokio_1 as tokio;

use futures_util::TryStreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os();
    let d = if args.len() > 1 {
        evdev::Device::open(&args.nth(1).unwrap())?
    } else {
        let mut devices = evdev::enumerate().collect::<Vec<_>>();
        for (i, d) in devices.iter().enumerate() {
            println!("{}: {}", i, d.name().unwrap_or("Unnamed device"));
        }
        print!("Select the device [0-{}]: ", devices.len());
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut chosen = String::new();
        std::io::stdin().read_line(&mut chosen)?;
        devices.swap_remove(chosen.trim().parse::<usize>()?)
    };
    println!("{}", d);
    println!("Events:");
    let mut events = d.into_event_stream_no_sync()?;
    while let Some(ev) = events.try_next().await? {
        println!("{:?}", ev);
    }
    println!("EOF!");
    Ok(())
}

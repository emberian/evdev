use evdev::{FFEffectData, FFEffectKind, FFReplay, FFTrigger};

mod _pick_device;

fn main() -> std::io::Result<()> {
    let mut d = _pick_device::pick_device();
    println!("{}", d);
    println!("It's time to rumble!");

    let effect_data = FFEffectData {
        direction: 0,
        trigger: FFTrigger::default(),
        replay: FFReplay {
            length: 1000,
            delay: 0,
        },
        kind: FFEffectKind::Rumble {
            strong_magnitude: 0x0000,
            weak_magnitude: 0xffff,
        },
    };

    let mut effect = d.upload_ff_effect(effect_data)?;

    effect.play(1)?;
    std::thread::sleep(std::time::Duration::from_secs(1));
    effect.stop()?;
    std::thread::sleep(std::time::Duration::from_secs(1));

    Ok(())
}

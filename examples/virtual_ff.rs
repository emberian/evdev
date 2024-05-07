// Create a virtual force feedback device, just while this is running.

use evdev::{
    uinput::VirtualDeviceBuilder, AttributeSet, EventSummary, FFEffectCode, FFStatusCode,
    InputEvent, UInputCode,
};
use std::collections::BTreeSet;

fn main() -> std::io::Result<()> {
    let mut device = VirtualDeviceBuilder::new()?
        .name("Fake Force Feedback")
        .with_ff(&AttributeSet::from_iter([FFEffectCode::FF_RUMBLE]))?
        .with_ff_effects_max(16)
        .build()?;

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    let mut ids: BTreeSet<u16> = (0..16).collect();

    println!("Waiting for Ctrl-C...");
    loop {
        let events: Vec<InputEvent> = device.fetch_events()?.collect();

        const STOPPED: i32 = FFStatusCode::FF_STATUS_STOPPED.0 as i32;
        const PLAYING: i32 = FFStatusCode::FF_STATUS_PLAYING.0 as i32;

        for event in events {
            match event.destructure() {
                EventSummary::UInput(event, UInputCode::UI_FF_UPLOAD, ..) => {
                    let mut event = device.process_ff_upload(event)?;
                    let id = ids.iter().next().copied();
                    match id {
                        Some(id) => {
                            ids.remove(&id);
                            event.set_effect_id(id as i16);
                            event.set_retval(0);
                        }
                        None => {
                            event.set_retval(-1);
                        }
                    }
                    println!("upload effect {:?}", event.effect());
                }
                EventSummary::UInput(event, UInputCode::UI_FF_ERASE, ..) => {
                    let event = device.process_ff_erase(event)?;
                    ids.insert(event.effect_id() as u16);
                    println!("erase effect ID = {}", event.effect_id());
                }
                EventSummary::ForceFeedback(.., effect_id, STOPPED) => {
                    println!("stopped effect ID = {}", effect_id.0);
                }
                EventSummary::ForceFeedback(.., effect_id, PLAYING) => {
                    println!("playing effect ID = {}", effect_id.0);
                }
                _ => {
                    println!("event = {:?}", event);
                }
            };
        }
    }
}

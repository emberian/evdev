// Create a virtual force feedback device, just while this is running.

use evdev::{
    uinput::VirtualDeviceBuilder, UInputType,AttributeSet, Error, 
    EvdevEnum, FFEffectType, FFStatusType, InputEventKind, InputEvent, EvdevEvent,
};
use std::collections::BTreeSet;

fn main() -> Result<(), Error> {
    let mut device = VirtualDeviceBuilder::new()?
        .name("Fake Force Feedback")
        .with_ff(&AttributeSet::from_iter([FFEffectType::FF_RUMBLE]))?
        .with_ff_effects_max(16)
        .build()?;

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    let mut ids: BTreeSet<u16> = (0..16).into_iter().collect();

    println!("Waiting for Ctrl-C...");
    loop {
        let events: Vec<InputEvent> = device.fetch_events()?.collect();

        for event in events {
             match event.kind() {
                InputEventKind::UInput(event,  UInputType::UI_FF_UPLOAD) => {
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
                },
                InputEventKind::UInput(event,UInputType::UI_FF_ERASE) => {
                    let event = device.process_ff_erase(event)?;
                    ids.insert(event.effect_id() as u16);
                    println!("erase effect ID = {}", event.effect_id());
                },
                InputEventKind::ForceFeedback(event, effect_id) => {
                    let status = FFStatusType::from_index(event.value() as usize);

                    match status {
                        FFStatusType::FF_STATUS_STOPPED => {
                            println!("stopped effect ID = {}", effect_id.0);
                        }
                        FFStatusType::FF_STATUS_PLAYING => {
                            println!("playing effect ID = {}", effect_id.0);
                        }
                        _ => (),
                    }
                },
                kind => {
                    println!("event kind = {:?}", kind);
                },
            };
        }
    }
}

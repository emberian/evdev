// Create a virtual force feedback device, just while this is running.

use evdev::{
    uinput::{UInputEvent, VirtualDeviceBuilder},
    AttributeSet, Error, EvdevEnum, FFEffectType, FFStatus, InputEventKind, UInputEventType,
};
use std::collections::BTreeSet;

fn main() -> Result<(), Error> {
    let mut device = VirtualDeviceBuilder::new()?
        .name("Fake Force Feedback")
        .with_ff(&AttributeSet::from_iter([FFEffectType::FF_RUMBLE]))?
        .with_ff_effects_max(16)
        .build()?;

    let mut ids: BTreeSet<u16> = (0..16).into_iter().collect();

    println!("Waiting for Ctrl-C...");
    loop {
        let events: Vec<UInputEvent> = device.fetch_events()?.collect();

        for event in events {
            let code = match event.kind() {
                InputEventKind::UInput(code) => UInputEventType::from_index(code as usize),
                InputEventKind::ForceFeedback(effect_id) => {
                    let value = FFStatus::from_index(event.value() as usize);

                    match value {
                        FFStatus::FF_STATUS_STOPPED => {
                            println!("stopped effect ID = {}", effect_id);
                        }
                        FFStatus::FF_STATUS_PLAYING => {
                            println!("playing effect ID = {}", effect_id);
                        }
                        _ => (),
                    }

                    continue;
                }
                kind => {
                    println!("event kind = {:?}", kind);
                    continue;
                }
            };

            match code {
                UInputEventType::UI_FF_UPLOAD => {
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
                UInputEventType::UI_FF_ERASE => {
                    let event = device.process_ff_erase(event)?;

                    ids.insert(event.effect_id() as u16);

                    println!("erase effect ID = {}", event.effect_id());
                }
                _ => {
                    println!("event code {}", event.code());
                }
            }
        }
    }
}

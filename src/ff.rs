use crate::attribute_set::EvdevEnum;
use crate::constants::FFEffectType;
use crate::sys;

/// Describes a generic force feedback effect envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FFEnvelope {
    /// How long the attack should last in milliseconds.
    pub attack_length: u16,
    /// The level of the attack at the beginning of the attack.
    pub attack_level: u16,
    /// How long the fade should last in milliseconds.
    pub fade_length: u16,
    /// The level of the fade at the end of the fade.
    pub fade_level: u16,
}

impl From<libc::ff_envelope> for FFEnvelope {
    fn from(value: libc::ff_envelope) -> Self {
        Self {
            attack_length: value.attack_length,
            attack_level: value.attack_level,
            fade_length: value.fade_length,
            fade_level: value.fade_level,
        }
    }
}

impl Into<libc::ff_envelope> for FFEnvelope {
    fn into(self) -> libc::ff_envelope {
        libc::ff_envelope {
            attack_length: self.attack_length,
            attack_level: self.attack_level,
            fade_length: self.fade_length,
            fade_level: self.fade_level,
        }
    }
}

/// Describes the waveform for periodic force feedback effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FFWaveform {
    /// Square waveform.
    Square,
    /// Triangle waveform.
    Triangle,
    /// Sine waveform.
    Sine,
    /// Sawtooth up waveform.
    SawUp,
    /// Sawtooth down waveform.
    SawDown,
}

impl Into<FFEffectType> for FFWaveform {
    fn into(self) -> FFEffectType {
        match self {
            Self::Square => FFEffectType::FF_SQUARE,
            Self::Triangle => FFEffectType::FF_TRIANGLE,
            Self::Sine => FFEffectType::FF_SINE,
            Self::SawUp => FFEffectType::FF_SAW_UP,
            Self::SawDown => FFEffectType::FF_SAW_DOWN,
        }
    }
}

/// Describes a spring or friction force feedback effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FFCondition {
    /// The maximum level when the joystick is moved all the way to the right.
    pub right_saturation: u16,
    /// The maximum level when the joystick is moved all the way to the left.
    pub left_saturation: u16,
    /// The coefficient that controls how fast the force grows when the joystick moves to the
    /// right.
    pub right_coefficient: i16,
    /// The coefficient that controls how fast the force grows when the joystick moves to the left.
    pub left_coefficient: i16,
    /// The size of the dead zone, which is the zone where no force is produced.
    pub deadband: u16,
    /// The position of the dead zone.
    pub center: i16,
}

impl From<libc::ff_condition_effect> for FFCondition {
    fn from(value: libc::ff_condition_effect) -> Self {
        Self {
            right_saturation: value.right_saturation,
            left_saturation: value.left_saturation,
            right_coefficient: value.right_coeff,
            left_coefficient: value.left_coeff,
            deadband: value.deadband,
            center: value.center,
        }
    }
}

impl Into<libc::ff_condition_effect> for FFCondition {
    fn into(self) -> libc::ff_condition_effect {
        libc::ff_condition_effect {
            right_saturation: self.right_saturation,
            left_saturation: self.left_saturation,
            right_coeff: self.right_coefficient,
            left_coeff: self.left_coefficient,
            deadband: self.deadband,
            center: self.center,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FFEffectKind {
    Damper,
    Inertia,
    Constant {
        /// The strength of the effect.
        level: i16,
        /// Envelope data.
        envelope: FFEnvelope,
    },
    Ramp {
        /// The strength at the beginning of the effect.
        start_level: i16,
        /// The strength at the end of the effect.
        end_level: i16,
        /// Envelope data.
        envelope: FFEnvelope,
    },
    Periodic {
        /// The kind of waveform to use for the force feedback effect.
        waveform: FFWaveform,
        /// The period of the wave in milliseconds.
        period: u16,
        /// The peak value or amplitude of the wave.
        magnitude: i16,
        /// The mean value of the wave (roughly).
        offset: i16,
        /// The horizontal shift.
        phase: u16,
        /// Envelope data.
        envelope: FFEnvelope,
    },
    Spring {
        /// Condition data for each axis.
        condition: [FFCondition; 2],
    },
    Friction {
        /// Condition data for each axis.
        condition: [FFCondition; 2],
    },
    Rumble {
        /// The magnitude of the heavy motor.
        strong_magnitude: u16,
        /// The magnitude of the light motor.
        weak_magnitude: u16,
    },
}

impl Into<FFEffectType> for FFEffectKind {
    fn into(self) -> FFEffectType {
        match self {
            Self::Damper => FFEffectType::FF_DAMPER,
            Self::Inertia => FFEffectType::FF_INERTIA,
            Self::Constant { .. } => FFEffectType::FF_CONSTANT,
            Self::Ramp { .. } => FFEffectType::FF_RAMP,
            Self::Periodic { .. } => FFEffectType::FF_PERIODIC,
            Self::Spring { .. } => FFEffectType::FF_SPRING,
            Self::Friction { .. } => FFEffectType::FF_FRICTION,
            Self::Rumble { .. } => FFEffectType::FF_RUMBLE,
        }
    }
}

/// Trigger information for the force feedback effect.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FFTrigger {
    /// The button number that triggers the force feedback effect.
    pub button: u16,
    /// How long to wait before the force feedback effect can be triggered again in milliseconds.
    pub interval: u16,
}

impl From<libc::ff_trigger> for FFTrigger {
    fn from(value: libc::ff_trigger) -> Self {
        Self {
            button: value.button,
            interval: value.interval,
        }
    }
}

impl Into<libc::ff_trigger> for FFTrigger {
    fn into(self) -> libc::ff_trigger {
        libc::ff_trigger {
            button: self.button,
            interval: self.interval,
        }
    }
}

/// Scheduling information for the force feedback effect.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FFReplay {
    /// How long the force feedback effect should last in milliseconds.
    pub length: u16,
    /// How long to wait before the force feedback effect should play in milliseconds.
    pub delay: u16,
}

impl From<libc::ff_replay> for FFReplay {
    fn from(value: libc::ff_replay) -> Self {
        Self {
            length: value.length,
            delay: value.delay,
        }
    }
}

impl Into<libc::ff_replay> for FFReplay {
    fn into(self) -> libc::ff_replay {
        libc::ff_replay {
            length: self.length,
            delay: self.delay,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FFEffectData {
    /// The direction of the force feedback effect.
    pub direction: u16,
    /// Trigger conditions.
    pub trigger: FFTrigger,
    /// Scheduling of the effect.
    pub replay: FFReplay,
    /// The type of force feedback effect and any associated parameters.
    pub kind: FFEffectKind,
}

impl From<sys::ff_effect> for FFEffectData {
    fn from(value: sys::ff_effect) -> Self {
        let kind = match FFEffectType::from_index(value.type_ as usize) {
            FFEffectType::FF_DAMPER => FFEffectKind::Damper,
            FFEffectType::FF_INERTIA => FFEffectKind::Inertia,
            FFEffectType::FF_CONSTANT => {
                let constant = unsafe { value.u.constant };

                FFEffectKind::Constant {
                    level: constant.level,
                    envelope: constant.envelope.into(),
                }
            }
            FFEffectType::FF_RAMP => {
                let ramp = unsafe { value.u.ramp };

                FFEffectKind::Ramp {
                    start_level: ramp.start_level,
                    end_level: ramp.end_level,
                    envelope: ramp.envelope.into(),
                }
            }
            FFEffectType::FF_PERIODIC => {
                let periodic = unsafe { value.u.periodic };

                FFEffectKind::Periodic {
                    waveform: match FFEffectType::from_index(periodic.waveform as usize) {
                        FFEffectType::FF_SQUARE => FFWaveform::Square,
                        FFEffectType::FF_TRIANGLE => FFWaveform::Triangle,
                        FFEffectType::FF_SINE => FFWaveform::Sine,
                        FFEffectType::FF_SAW_UP => FFWaveform::SawUp,
                        FFEffectType::FF_SAW_DOWN => FFWaveform::SawDown,
                        _ => unreachable!(),
                    },
                    period: periodic.period,
                    magnitude: periodic.magnitude,
                    offset: periodic.offset,
                    phase: periodic.phase,
                    envelope: periodic.envelope.into(),
                }
            }
            FFEffectType::FF_SPRING => {
                let condition = unsafe { value.u.condition };

                FFEffectKind::Spring {
                    condition: [condition[0].into(), condition[1].into()],
                }
            }
            FFEffectType::FF_FRICTION => {
                let condition = unsafe { value.u.condition };

                FFEffectKind::Friction {
                    condition: [condition[0].into(), condition[1].into()],
                }
            }
            FFEffectType::FF_RUMBLE => {
                let rumble = unsafe { value.u.rumble };

                FFEffectKind::Rumble {
                    strong_magnitude: rumble.strong_magnitude,
                    weak_magnitude: rumble.weak_magnitude,
                }
            }
            _ => unreachable!(),
        };

        Self {
            direction: value.direction,
            trigger: value.trigger.into(),
            replay: value.replay.into(),
            kind,
        }
    }
}

impl Into<sys::ff_effect> for FFEffectData {
    fn into(self) -> sys::ff_effect {
        let mut effect: sys::ff_effect = unsafe { std::mem::zeroed() };

        let type_: FFEffectType = self.kind.into();
        effect.type_ = type_.0;
        effect.direction = self.direction;
        effect.trigger = self.trigger.into();
        effect.replay = self.replay.into();

        match self.kind {
            FFEffectKind::Constant { level, envelope } => {
                effect.u.constant.level = level;
                effect.u.constant.envelope = envelope.into();
            }
            FFEffectKind::Ramp {
                start_level,
                end_level,
                envelope,
            } => {
                effect.u.ramp.start_level = start_level;
                effect.u.ramp.end_level = end_level;
                effect.u.ramp.envelope = envelope.into();
            }
            FFEffectKind::Periodic {
                waveform,
                period,
                magnitude,
                offset,
                phase,
                envelope,
            } => {
                let waveform: FFEffectType = waveform.into();
                effect.u.periodic.waveform = waveform.0;
                effect.u.periodic.period = period;
                effect.u.periodic.magnitude = magnitude;
                effect.u.periodic.offset = offset;
                effect.u.periodic.phase = phase;
                effect.u.periodic.envelope = envelope.into();
            }
            FFEffectKind::Spring { condition } | FFEffectKind::Friction { condition } => {
                effect.u.condition = [condition[0].into(), condition[1].into()];
            }
            FFEffectKind::Rumble {
                strong_magnitude,
                weak_magnitude,
            } => {
                effect.u.rumble.strong_magnitude = strong_magnitude;
                effect.u.rumble.weak_magnitude = weak_magnitude;
            }
            _ => (),
        }

        effect
    }
}

use crate::compat::input_id;
use std::fmt;

#[derive(Clone)]
#[repr(transparent)]
pub struct InputId(pub(crate) input_id);

impl From<input_id> for InputId {
    #[inline]
    fn from(id: input_id) -> Self {
        Self(id)
    }
}
impl AsRef<input_id> for InputId {
    #[inline]
    fn as_ref(&self) -> &input_id {
        &self.0
    }
}

impl InputId {
    pub fn bus_type(&self) -> BusType {
        BusType(self.0.bustype)
    }
    pub fn vendor(&self) -> u16 {
        self.0.vendor
    }
    pub fn product(&self) -> u16 {
        self.0.product
    }
    pub fn version(&self) -> u16 {
        self.0.version
    }

    /// Crate a new InputId, useful for customizing virtual input devices.
    pub fn new(bus_type: BusType, vendor: u16, product: u16, version: u16) -> Self {
        Self::from(input_id {
            bustype: bus_type.0,
            vendor,
            product,
            version,
        })
    }
}

impl fmt::Debug for InputId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InputId")
            .field("bus_type", &self.bus_type())
            .field("vendor", &format_args!("{:#x}", self.vendor()))
            .field("product", &format_args!("{:#x}", self.product()))
            .field("version", &format_args!("{:#x}", self.version()))
            .finish()
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct BusType(pub u16);

evdev_enum!(
    BusType,
    BUS_PCI = 0x01,
    BUS_ISAPNP = 0x02,
    BUS_USB = 0x03,
    BUS_HIL = 0x04,
    BUS_BLUETOOTH = 0x05,
    BUS_VIRTUAL = 0x06,
    BUS_ISA = 0x10,
    BUS_I8042 = 0x11,
    BUS_XTKBD = 0x12,
    BUS_RS232 = 0x13,
    BUS_GAMEPORT = 0x14,
    BUS_PARPORT = 0x15,
    BUS_AMIGA = 0x16,
    BUS_ADB = 0x17,
    BUS_I2C = 0x18,
    BUS_HOST = 0x19,
    BUS_GSC = 0x1A,
    BUS_ATARI = 0x1B,
    BUS_SPI = 0x1C,
    BUS_RMI = 0x1D,
    BUS_CEC = 0x1E,
    BUS_INTEL_ISHTP = 0x1F,
);

impl fmt::Display for BusType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            Self::BUS_PCI => "PCI",
            Self::BUS_ISAPNP => "ISA Plug 'n Play",
            Self::BUS_USB => "USB",
            Self::BUS_HIL => "HIL",
            Self::BUS_BLUETOOTH => "Bluetooth",
            Self::BUS_VIRTUAL => "Virtual",
            Self::BUS_ISA => "ISA",
            Self::BUS_I8042 => "i8042",
            Self::BUS_XTKBD => "XTKBD",
            Self::BUS_RS232 => "RS232",
            Self::BUS_GAMEPORT => "Gameport",
            Self::BUS_PARPORT => "Parallel Port",
            Self::BUS_AMIGA => "Amiga",
            Self::BUS_ADB => "ADB",
            Self::BUS_I2C => "I2C",
            Self::BUS_HOST => "Host",
            Self::BUS_GSC => "GSC",
            Self::BUS_ATARI => "Atari",
            Self::BUS_SPI => "SPI",
            Self::BUS_RMI => "RMI",
            Self::BUS_CEC => "CEC",
            Self::BUS_INTEL_ISHTP => "Intel ISHTP",
            _ => "Unknown",
        };
        f.write_str(s)
    }
}

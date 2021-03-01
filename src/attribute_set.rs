use bitvec::prelude::*;
use std::fmt;

/// A collection of bits representing either device capability or state.
///
/// This can be used to iterate across all keys supported by a keyboard, or all buttons supported
/// by a joystick. You can also query directly whether a specific bit is set (corresponding to
/// whether a key or button is depressed).
#[derive(Copy, Clone)]
pub struct AttributeSet<'a, T> {
    bitslice: &'a BitSlice<Lsb0, u8>,
    _indexer: std::marker::PhantomData<T>,
}

impl<'a, T: EvdevEnum> AttributeSet<'a, T> {
    #[inline]
    pub(crate) fn new(bitslice: &'a BitSlice<Lsb0, u8>) -> Self {
        Self {
            bitslice,
            _indexer: std::marker::PhantomData,
        }
    }

    #[inline]
    /// Returns `true` if this AttributeSet contains the passed T.
    pub fn contains(&self, attr: T) -> bool {
        self.bitslice.get(attr.to_index()).map_or(false, |b| *b)
    }

    #[inline]
    /// Provides an iterator over all "set" bits in the collection.
    pub fn iter(&self) -> impl Iterator<Item = T> + 'a {
        self.bitslice.iter_ones().map(T::from_index)
    }
}

impl<'a, T: EvdevEnum + fmt::Debug> fmt::Debug for AttributeSet<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

pub trait EvdevEnum: Copy + 'static {
    fn from_index(i: usize) -> Self;
    fn to_index(self) -> usize;
}

macro_rules! evdev_enum {
    ($t:ty, $($(#[$attr:meta])* $c:ident = $val:expr,)*) => {
        impl $t {
            $($(#[$attr])* pub const $c: Self = Self($val);)*
        }
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                match *self {
                    $(Self::$c => f.pad(stringify!($c)),)*
                    _ => write!(f, "unknown key: {}", self.0),
                }
            }
        }
        impl $crate::attribute_set::EvdevEnum for $t {
            #[inline]
            fn from_index(i: usize) -> Self {
                Self(i as _)
            }
            #[inline]
            fn to_index(self) -> usize {
                self.0 as _
            }
        }
    }
}

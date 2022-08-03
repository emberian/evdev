use bitvec::prelude::*;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// A collection of bits representing either device capability or state.
///
/// This can be used to iterate across all keys supported by a keyboard, or all buttons supported
/// by a joystick. You can also query directly whether a specific bit is set (corresponding to
/// whether a key or button is depressed).
#[repr(transparent)]
pub struct AttributeSetRef<T> {
    _indexer: std::marker::PhantomData<T>,
    bitslice: BitSlice<u8>,
}

impl<T: EvdevEnum> AttributeSetRef<T> {
    #[inline]
    fn new(bitslice: &BitSlice<u8>) -> &Self {
        // SAFETY: for<T> AttributeSet<T> is repr(transparent) over BitSlice<u8>
        unsafe { &*(bitslice as *const BitSlice<u8> as *const Self) }
    }

    #[inline]
    fn new_mut(bitslice: &mut BitSlice<u8>) -> &mut Self {
        // SAFETY: for<T> AttributeSet<T> is repr(transparent) over BitSlice<u8>
        unsafe { &mut *(bitslice as *mut BitSlice<u8> as *mut Self) }
    }

    /// Returns `true` if this AttributeSet contains the passed T.
    #[inline]
    pub fn contains(&self, attr: T) -> bool {
        self.bitslice.get(attr.to_index()).map_or(false, |b| *b)
    }

    /// Provides an iterator over all "set" bits in the collection.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.bitslice.iter_ones().map(T::from_index)
    }

    #[inline]
    pub(crate) fn slice(&self, start: T) -> &Self {
        Self::new(&self.bitslice[start.to_index()..])
    }

    pub fn insert(&mut self, attr: T) {
        self.set(attr, true)
    }

    pub fn remove(&mut self, attr: T) {
        self.set(attr, false)
    }

    // TODO: figure out a good name for this if we make it public
    #[inline]
    pub(crate) fn set(&mut self, attr: T, on: bool) {
        self.bitslice.set(attr.to_index(), on)
    }
}

impl<T: EvdevEnum + fmt::Debug> fmt::Debug for AttributeSetRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

pub struct AttributeSet<T: ArrayedEvdevEnum> {
    container: T::Array,
}

impl<T: ArrayedEvdevEnum> AttributeSet<T> {
    pub fn new() -> Self {
        Self {
            container: T::zeroed_array(),
        }
    }

    fn as_bitslice(&self) -> &BitSlice<u8> {
        T::array_as_slice(&self.container)
    }

    fn as_mut_bitslice(&mut self) -> &mut BitSlice<u8> {
        T::array_as_slice_mut(&mut self.container)
    }

    #[inline]
    pub(crate) fn as_mut_raw_slice(&mut self) -> &mut [u8] {
        T::array_as_buf(&mut self.container)
    }
}

impl<T: ArrayedEvdevEnum> Default for AttributeSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ArrayedEvdevEnum> std::iter::FromIterator<T> for AttributeSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = AttributeSet::default();
        iter.into_iter().for_each(|el| set.insert(el));
        set
    }
}

impl<T: ArrayedEvdevEnum> Deref for AttributeSet<T> {
    type Target = AttributeSetRef<T>;
    fn deref(&self) -> &AttributeSetRef<T> {
        AttributeSetRef::new(self.as_bitslice())
    }
}

impl<T: ArrayedEvdevEnum> DerefMut for AttributeSet<T> {
    fn deref_mut(&mut self) -> &mut AttributeSetRef<T> {
        AttributeSetRef::new_mut(self.as_mut_bitslice())
    }
}

impl<T: ArrayedEvdevEnum> Clone for AttributeSet<T>
where
    T::Array: Clone,
{
    fn clone(&self) -> Self {
        Self {
            container: self.container.clone(),
        }
    }
    fn clone_from(&mut self, other: &Self) {
        self.container.clone_from(&other.container)
    }
}

impl<T: ArrayedEvdevEnum + fmt::Debug> fmt::Debug for AttributeSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

pub trait EvdevEnum: Copy + 'static {
    fn from_index(i: usize) -> Self;
    fn to_index(self) -> usize;
}

pub trait ArrayedEvdevEnum: EvdevEnum {
    type Array;
    fn array_as_slice(arr: &Self::Array) -> &BitSlice<u8>;
    fn array_as_slice_mut(arr: &mut Self::Array) -> &mut BitSlice<u8>;
    fn array_as_buf(arr: &mut Self::Array) -> &mut [u8];
    fn zeroed_array() -> Self::Array;
}

macro_rules! evdev_enum {
    ($t:ty, Array, $($(#[$attr:meta])* $c:ident = $val:expr,)*) => {
        evdev_enum!(
            $t,
            Array: bitvec::BitArr!(for <$t>::COUNT, in u8),
            bitvec::array::BitArray::as_raw_mut_slice,
            bitvec::array::BitArray::ZERO,
            $($(#[$attr])* $c = $val,)*
        );
    };
    ($t:ty, box Array, $($(#[$attr:meta])* $c:ident = $val:expr,)*) => {
        evdev_enum!(
            $t,
            Array: Box<bitvec::BitArr!(for <$t>::COUNT, in u8)>,
            bitvec::array::BitArray::as_raw_mut_slice,
            Box::new(bitvec::array::BitArray::ZERO),
            $($(#[$attr])* $c = $val,)*
        );
    };
    (
        $t:ty,
        Array: $Array:ty, $arr_as_buf:expr, $zero:expr,
        $($(#[$attr:meta])* $c:ident = $val:expr,)*
    ) => {
        impl $crate::attribute_set::ArrayedEvdevEnum for $t {
            type Array = $Array;
            fn array_as_slice(arr: &Self::Array) -> &bitvec::slice::BitSlice<u8> {
                arr
            }
            fn array_as_slice_mut(arr: &mut Self::Array) -> &mut bitvec::slice::BitSlice<u8> {
                arr
            }
            fn array_as_buf(arr: &mut Self::Array) -> &mut [u8] {
                $arr_as_buf(arr)
            }
            fn zeroed_array() -> Self::Array {
                $zero
            }
        }
        evdev_enum!($t, $($(#[$attr])* $c = $val,)*);
    };
    ($t:ty, $($(#[$attr:meta])* $c:ident = $val:expr,)*) => {
        impl $t {
            $($(#[$attr])* pub const $c: Self = Self($val);)*
        }
        impl std::str::FromStr for $t {
            type Err = crate::EnumParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let map: &[(&'static str, $t)] = &[
                    $((stringify!($c), Self::$c),)*
                ];

                match map.iter().find(|e| e.0 == s) {
                    Some(e) => Ok(e.1),
                    None => Err(crate::EnumParseError(())),
                }
            }
        }
        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                #[allow(unreachable_patterns)]
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
        #[cfg(feature = "serde")]
        #[allow(unreachable_patterns)]
        impl serde_1::Serialize for $t {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde_1::ser::Serializer,
            {
                let value = match *self {
                    $(Self::$c => stringify!($c),)*
                    _ => unreachable!(),
                };

                serializer.serialize_str(value)
            }
        }
        #[cfg(feature = "serde")]
        paste::paste! {
            struct [<$t Visitor>];
            impl<'de> serde_1::de::Visitor<'de> for [<$t Visitor>] {
                type Value = $t;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(formatter, "a string with any of the constants in {}", stringify!($t))
                }

                fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                where
                    E: serde_1::de::Error,
                {
                    match s.to_lowercase().as_str() {
                        $(stringify!([<$c:lower>]) => Ok($t::$c),)*
                        _ => Err(serde_1::de::Error::invalid_value(serde_1::de::Unexpected::Str(s), &self)),
                    }
                }
            }
            impl<'de> serde_1::Deserialize<'de> for $t {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde_1::de::Deserializer<'de>,
                {
                    deserializer.deserialize_str([<$t Visitor>])
                }
            }
        }
    }
}

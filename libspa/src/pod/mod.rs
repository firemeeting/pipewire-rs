//! This module deals with SPA pods, providing ways to represent pods using idiomatic types
//! and serialize them into their raw representation, and the other way around.
//!
//! Everything concerning serializing raw pods from rust types is in the [`serialize`] submodule.
//! and everything about deserializing rust types from raw pods is in the [`deserialize`] submodule.
//!
//! The entire serialization and deserialization approach is inspired by and similar to the excellent `serde` crate,
//! but is much more specialized to fit the SPA pod format.

pub mod deserialize;
pub mod parser;
pub mod serialize;

use std::{
    ffi::c_void,
    io::{Seek, Write},
    mem::MaybeUninit,
    os::fd::RawFd,
};

use bitflags::bitflags;
use cookie_factory::{
    bytes::{ne_f32, ne_f64, ne_i32, ne_i64, ne_u32},
    gen_simple,
    sequence::pair,
    GenError,
};
use nix::errno::Errno;
use nom::{
    combinator::map,
    number::{
        complete::{f32, f64, i32, i64, u32},
        Endianness,
    },
    IResult,
};

use deserialize::{BoolVisitor, NoneVisitor, PodDeserialize, PodDeserializer};
use serialize::{PodSerialize, PodSerializer};

use crate::utils::{Choice, Fd, Fraction, Id, Rectangle};

use self::deserialize::{
    ChoiceDoubleVisitor, ChoiceFdVisitor, ChoiceFloatVisitor, ChoiceFractionVisitor,
    ChoiceIdVisitor, ChoiceIntVisitor, ChoiceLongVisitor, ChoiceRectangleVisitor, DoubleVisitor,
    FdVisitor, FloatVisitor, FractionVisitor, IdVisitor, IntVisitor, LongVisitor, PointerVisitor,
    RectangleVisitor,
};

/// A transparent wrapper around a `spa_sys::spa_pod`.
#[repr(transparent)]
pub struct Pod(spa_sys::spa_pod);

impl Pod {
    /// # Safety
    ///
    /// The provided pointer must point to a valid, well-aligned pod.
    ///
    /// The pods allocation must fit the entire size of the pod as indicated
    /// by the pods header, including header size, body size and any padding.
    ///
    /// The provided pod must not be mutated, moved, freed or similar while
    /// the borrow returned from this function is in use.
    /// This also means that other nonmutable borrows may be created to this pod,
    /// but no mutable borrows to this pod may be created until all borrows are dropped.
    ///
    /// The returned type has `'static` lifetime.
    /// It is suggested to shorten the lifetime to whatever is applicable afterwards.
    pub unsafe fn from_raw(pod: *const spa_sys::spa_pod) -> &'static Self {
        pod.cast::<Self>().as_ref().unwrap()
    }

    /// # Safety
    ///
    /// The provided pointer must point to a valid, well-aligned pod.
    ///
    /// The pods allocation must fit the entire size of the pod as indicated
    /// by the pods header, including header size, body size and any padding.
    ///
    /// The provided pod must not be mutated, moved, freed or similar while
    /// the borrow returned from this function is in use.
    /// This also means that no other borrow to this pod may be created until the borrow is dropped.
    ///
    /// The returned type has `'static` lifetime.
    /// It is suggested to shorten the lifetime to whatever is applicable afterwards.
    pub unsafe fn from_raw_mut(pod: *mut spa_sys::spa_pod) -> &'static mut Self {
        pod.cast::<Self>().as_mut().unwrap()
    }

    pub fn as_raw_ptr(&self) -> *mut spa_sys::spa_pod {
        std::ptr::addr_of!(self.0).cast_mut()
    }

    // TODO: Other methods from iter.h that are still missing

    pub fn is_none(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_none(self.as_raw_ptr()) };
        res != 0
    }

    pub fn is_bool(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_bool(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_bool(&self) -> Result<bool, Errno> {
        unsafe {
            let mut b: MaybeUninit<bool> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_bool(self.as_raw_ptr(), b.as_mut_ptr());

            if res >= 0 {
                Ok(b.assume_init())
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_id(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_id(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_id(&self) -> Result<Id, Errno> {
        unsafe {
            let mut id: MaybeUninit<u32> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_id(self.as_raw_ptr(), id.as_mut_ptr());

            if res >= 0 {
                Ok(Id(id.assume_init()))
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_int(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_int(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_int(&self) -> Result<i32, Errno> {
        unsafe {
            let mut int: MaybeUninit<i32> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_int(self.as_raw_ptr(), int.as_mut_ptr());

            if res >= 0 {
                Ok(int.assume_init())
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_long(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_long(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_long(&self) -> Result<i64, Errno> {
        unsafe {
            let mut long: MaybeUninit<i64> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_long(self.as_raw_ptr(), long.as_mut_ptr());

            if res >= 0 {
                Ok(long.assume_init())
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_float(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_float(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_float(&self) -> Result<f32, Errno> {
        unsafe {
            let mut float: MaybeUninit<f32> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_float(self.as_raw_ptr(), float.as_mut_ptr());

            if res >= 0 {
                Ok(float.assume_init())
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_double(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_double(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_double(&self) -> Result<f64, Errno> {
        unsafe {
            let mut double: MaybeUninit<f64> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_double(self.as_raw_ptr(), double.as_mut_ptr());

            if res >= 0 {
                Ok(double.assume_init())
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_string(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_string(self.as_raw_ptr()) };
        res != 0
    }

    // TODO: to_string

    pub fn is_bytes(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_bytes(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_bytes(&self) -> Result<&[u8], Errno> {
        unsafe {
            let mut bytes: MaybeUninit<*const c_void> = MaybeUninit::uninit();
            let mut len: MaybeUninit<u32> = MaybeUninit::uninit();
            let res =
                spa_sys::spa_pod_get_bytes(self.as_raw_ptr(), bytes.as_mut_ptr(), len.as_mut_ptr());

            if res >= 0 {
                let bytes = bytes.assume_init();
                let len = len.assume_init();
                let bytes = std::slice::from_raw_parts(bytes.cast(), len.try_into().unwrap());
                Ok(bytes)
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_pointer(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_pointer(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_pointer(&self) -> Result<(*const c_void, Id), Errno> {
        unsafe {
            let mut _type: MaybeUninit<u32> = MaybeUninit::uninit();
            let mut pointer: MaybeUninit<*const c_void> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_pointer(
                self.as_raw_ptr(),
                _type.as_mut_ptr(),
                pointer.as_mut_ptr(),
            );

            if res >= 0 {
                let _type = Id(_type.assume_init());
                let pointer = pointer.assume_init();
                Ok((pointer, _type))
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_fd(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_fd(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_fd(&self) -> Result<RawFd, Errno> {
        unsafe {
            let mut fd: MaybeUninit<i64> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_fd(self.as_raw_ptr(), fd.as_mut_ptr());

            if res >= 0 {
                let fd = fd.assume_init();
                let fd: RawFd = fd.try_into().unwrap();
                Ok(fd)
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_rectangle(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_rectangle(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_rectangle(&self) -> Result<Rectangle, Errno> {
        unsafe {
            let mut rectangle: MaybeUninit<spa_sys::spa_rectangle> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_rectangle(self.as_raw_ptr(), rectangle.as_mut_ptr());

            if res >= 0 {
                Ok(rectangle.assume_init())
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_fraction(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_fraction(self.as_raw_ptr()) };
        res != 0
    }

    pub fn get_fraction(&self) -> Result<Fraction, Errno> {
        unsafe {
            let mut fraction: MaybeUninit<spa_sys::spa_fraction> = MaybeUninit::uninit();
            let res = spa_sys::spa_pod_get_fraction(self.as_raw_ptr(), fraction.as_mut_ptr());

            if res >= 0 {
                Ok(fraction.assume_init())
            } else {
                Err(Errno::from_i32(-res))
            }
        }
    }

    pub fn is_bitmap(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_bitmap(self.as_raw_ptr()) };
        res != 0
    }

    pub fn is_array(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_array(self.as_raw_ptr()) };
        res != 0
    }

    pub fn is_choice(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_choice(self.as_raw_ptr()) };
        res != 0
    }

    pub fn is_struct(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_struct(self.as_raw_ptr()) };
        res != 0
    }

    pub fn is_object(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_object(self.as_raw_ptr()) };
        res != 0
    }

    pub fn is_sequence(&self) -> bool {
        let res = unsafe { spa_sys::spa_pod_is_sequence(self.as_raw_ptr()) };
        res != 0
    }
}

/// Implementors of this trait are the canonical representation of a specific type of fixed sized SPA pod.
///
/// They can be used as an output type for [`FixedSizedPod`] implementors
/// and take care of the actual serialization/deserialization from/to the type of raw SPA pod they represent.
///
/// The trait is sealed, so it can't be implemented outside of this crate.
/// This is to ensure that no invalid pod can be serialized.
///
/// If you want to have your type convert from and to a fixed sized pod, implement [`FixedSizedPod`] instead and choose
/// a fitting implementor of this trait as the `CanonicalType` instead.
pub trait CanonicalFixedSizedPod: private::CanonicalFixedSizedPodSeal {
    /// The raw type this serializes into.
    #[doc(hidden)]
    const TYPE: u32;
    /// The size of the pods body.
    #[doc(hidden)]
    const SIZE: u32;
    #[doc(hidden)]
    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError>;
    #[doc(hidden)]
    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized;
}

mod private {
    /// This trait makes [`super::CanonicalFixedSizedPod`] a "sealed trait", which makes it impossible to implement
    /// ouside of this crate.
    pub trait CanonicalFixedSizedPodSeal {}
    impl CanonicalFixedSizedPodSeal for () {}
    impl CanonicalFixedSizedPodSeal for bool {}
    impl CanonicalFixedSizedPodSeal for i32 {}
    impl CanonicalFixedSizedPodSeal for i64 {}
    impl CanonicalFixedSizedPodSeal for f32 {}
    impl CanonicalFixedSizedPodSeal for f64 {}
    impl CanonicalFixedSizedPodSeal for super::Rectangle {}
    impl CanonicalFixedSizedPodSeal for super::Fraction {}
    impl CanonicalFixedSizedPodSeal for super::Id {}
    impl CanonicalFixedSizedPodSeal for super::Fd {}
}

impl<T: CanonicalFixedSizedPod + Copy> FixedSizedPod for T {
    type CanonicalType = Self;

    fn as_canonical_type(&self) -> Self::CanonicalType {
        *self
    }

    fn from_canonical_type(canonical: &Self::CanonicalType) -> Self {
        *canonical
    }
}

/// Serialize into a `None` type pod.
impl CanonicalFixedSizedPod for () {
    const TYPE: u32 = spa_sys::SPA_TYPE_None;
    const SIZE: u32 = 0;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        Ok(out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        Ok((input, ()))
    }
}

/// Serialize into a `Bool` type pod.
impl CanonicalFixedSizedPod for bool {
    const TYPE: u32 = spa_sys::SPA_TYPE_Bool;
    const SIZE: u32 = 4;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(ne_u32(u32::from(*self)), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        map(u32(Endianness::Native), |b| b != 0)(input)
    }
}

/// Serialize into a `Int` type pod.
impl CanonicalFixedSizedPod for i32 {
    const TYPE: u32 = spa_sys::SPA_TYPE_Int;
    const SIZE: u32 = 4;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(ne_i32(*self), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        i32(Endianness::Native)(input)
    }
}

/// Serialize into a `Long` type pod.
impl CanonicalFixedSizedPod for i64 {
    const TYPE: u32 = spa_sys::SPA_TYPE_Long;
    const SIZE: u32 = 8;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(ne_i64(*self), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        i64(Endianness::Native)(input)
    }
}

/// Serialize into a `Float` type pod.
impl CanonicalFixedSizedPod for f32 {
    const TYPE: u32 = spa_sys::SPA_TYPE_Float;
    const SIZE: u32 = 4;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(ne_f32(*self), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        f32(Endianness::Native)(input)
    }
}

/// Serialize into a `Double` type pod.
impl CanonicalFixedSizedPod for f64 {
    const TYPE: u32 = spa_sys::SPA_TYPE_Double;
    const SIZE: u32 = 8;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(ne_f64(*self), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        f64(Endianness::Native)(input)
    }
}

/// Serialize into a `Rectangle` type pod.
impl CanonicalFixedSizedPod for Rectangle {
    const TYPE: u32 = spa_sys::SPA_TYPE_Rectangle;
    const SIZE: u32 = 8;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(pair(ne_u32(self.width), ne_u32(self.height)), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        map(
            nom::sequence::pair(u32(Endianness::Native), u32(Endianness::Native)),
            |(width, height)| Rectangle { width, height },
        )(input)
    }
}

/// Serialize into a `Fraction` type pod.
impl CanonicalFixedSizedPod for Fraction {
    const TYPE: u32 = spa_sys::SPA_TYPE_Fraction;
    const SIZE: u32 = 8;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(pair(ne_u32(self.num), ne_u32(self.denom)), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        map(
            nom::sequence::pair(u32(Endianness::Native), u32(Endianness::Native)),
            |(num, denom)| Fraction { num, denom },
        )(input)
    }
}

impl CanonicalFixedSizedPod for Id {
    const TYPE: u32 = spa_sys::SPA_TYPE_Id;
    const SIZE: u32 = 4;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(ne_u32(self.0), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        map(u32(Endianness::Native), Id)(input)
    }
}

impl CanonicalFixedSizedPod for Fd {
    const TYPE: u32 = spa_sys::SPA_TYPE_Fd;
    const SIZE: u32 = 8;

    fn serialize_body<O: Write>(&self, out: O) -> Result<O, GenError> {
        gen_simple(ne_i64(self.0), out)
    }

    fn deserialize_body(input: &[u8]) -> IResult<&[u8], Self>
    where
        Self: Sized,
    {
        map(i64(Endianness::Native), Fd)(input)
    }
}

/// Implementors of this trait can be serialized into pods that always have the same size.
/// This lets them be used as elements in `Array` type SPA Pods.
///
/// Implementors of this automatically implement [`PodSerialize`].
///
/// Serialization is accomplished by having the type convert itself into/from the canonical representation of this pod,
/// e.g. `i32` for a `Int` type pod.
///
/// That type then takes care of the actual serialization.
///
/// See the [`CanonicalFixedSizedPod`] trait for a list of possible target types.
///
/// Which type to convert in is specified with the traits [`FixedSizedPod::CanonicalType`] type,
/// while the traits [`as_canonical_type`](`FixedSizedPod::as_canonical_type`)
/// and [`from_canonical_type`](`FixedSizedPod::from_canonical_type`) methods are responsible for the actual conversion.
///
/// # Examples
/// Implementing the trait on a `i32` newtype wrapper:
/// ```rust
/// use libspa::pod::FixedSizedPod;
///
/// struct Newtype(i32);
///
/// impl FixedSizedPod for Newtype {
///     // The pod we want to serialize into is a `Int` type pod, which has `i32` as it's canonical representation.
///     type CanonicalType = i32;
///
///     fn as_canonical_type(&self) -> Self::CanonicalType {
///         // Convert self to the canonical type.
///         self.0
///     }
///
///     fn from_canonical_type(canonical: &Self::CanonicalType) -> Self {
///         // Create a new Self instance from the canonical type.
///         Newtype(*canonical)
///     }
/// }
/// ```
pub trait FixedSizedPod {
    /// The canonical representation of the type of pod that should be serialized to/deserialized from.
    type CanonicalType: CanonicalFixedSizedPod;

    /// Convert `self` to the canonical type.
    fn as_canonical_type(&self) -> Self::CanonicalType;
    /// Convert the canonical type to `Self`.
    fn from_canonical_type(_: &Self::CanonicalType) -> Self;
}

impl<T: FixedSizedPod> PodSerialize for T {
    fn serialize<O: Write + Seek>(
        &self,
        serializer: PodSerializer<O>,
    ) -> Result<serialize::SerializeSuccess<O>, GenError> {
        serializer.serialized_fixed_sized_pod(self)
    }
}

impl<'de> PodDeserialize<'de> for () {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_none(NoneVisitor)
    }
}

impl<'de> PodDeserialize<'de> for bool {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_bool(BoolVisitor)
    }
}

impl<'de> PodDeserialize<'de> for i32 {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_int(IntVisitor)
    }
}

impl<'de> PodDeserialize<'de> for i64 {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_long(LongVisitor)
    }
}

impl<'de> PodDeserialize<'de> for f32 {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_float(FloatVisitor)
    }
}

impl<'de> PodDeserialize<'de> for f64 {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_double(DoubleVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Rectangle {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_rectangle(RectangleVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Fraction {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_fraction(FractionVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Id {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_id(IdVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Fd {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_fd(FdVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<i32> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceIntVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<i64> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceLongVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<f32> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceFloatVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<f64> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceDoubleVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<Id> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceIdVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<Rectangle> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceRectangleVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<Fraction> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceFractionVisitor)
    }
}

impl<'de> PodDeserialize<'de> for Choice<Fd> {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_choice(ChoiceFdVisitor)
    }
}

impl<'de, T> PodDeserialize<'de> for (u32, *const T) {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_pointer(PointerVisitor::<T>::default())
    }
}

impl<'de> PodDeserialize<'de> for Value {
    fn deserialize(
        deserializer: PodDeserializer<'de>,
    ) -> Result<
        (Self, deserialize::DeserializeSuccess<'de>),
        deserialize::DeserializeError<&'de [u8]>,
    >
    where
        Self: Sized,
    {
        deserializer.deserialize_any()
    }
}

/// A typed pod value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// no value or a NULL pointer.
    None,
    /// a boolean value.
    Bool(bool),
    /// an enumerated value.
    Id(Id),
    /// a 32 bits integer.
    Int(i32),
    /// a 64 bits integer.
    Long(i64),
    /// a 32 bits floating.
    Float(f32),
    /// a 64 bits floating.
    Double(f64),
    /// a string.
    String(String),
    /// a byte array.
    Bytes(Vec<u8>),
    /// a rectangle with width and height.
    Rectangle(Rectangle),
    /// a fraction with numerator and denominator.
    Fraction(Fraction),
    /// a file descriptor.
    Fd(Fd),
    /// an array of same type objects.
    ValueArray(ValueArray),
    /// a collection of types and objects.
    Struct(Vec<Value>),
    /// an object.
    Object(Object),
    /// a choice.
    Choice(ChoiceValue),
    /// a pointer.
    Pointer(u32, *const c_void),
}

/// an array of same type objects.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueArray {
    /// an array of none.
    None(Vec<()>),
    /// an array of booleans.
    Bool(Vec<bool>),
    /// an array of Id.
    Id(Vec<Id>),
    /// an array of 32 bits integer.
    Int(Vec<i32>),
    /// an array of 64 bits integer.
    Long(Vec<i64>),
    /// an array of 32 bits floating.
    Float(Vec<f32>),
    /// an array of 64 bits floating.
    Double(Vec<f64>),
    /// an array of Rectangle.
    Rectangle(Vec<Rectangle>),
    /// an array of Fraction.
    Fraction(Vec<Fraction>),
    /// an array of Fd.
    Fd(Vec<Fd>),
}

/// A typed choice.
#[derive(Debug, Clone, PartialEq)]
pub enum ChoiceValue {
    /// Choice on 32 bits integer values.
    Int(Choice<i32>),
    /// Choice on 64 bits integer values.
    Long(Choice<i64>),
    /// Choice on 32 bits floating values.
    Float(Choice<f32>),
    /// Choice on 64 bits floating values.
    Double(Choice<f64>),
    /// Choice on id values.
    Id(Choice<Id>),
    /// Choice on rectangle values.
    Rectangle(Choice<Rectangle>),
    /// Choice on fraction values.
    Fraction(Choice<Fraction>),
    /// Choice on fd values.
    Fd(Choice<Fd>),
}

/// An object from a pod.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    /// the object type.
    pub type_: u32,
    /// the object id.
    pub id: u32,
    /// the object properties.
    pub properties: Vec<Property>,
}

/// A macro for creating a new [`Object`] with properties.
///
/// The macro accepts the object type, id and a list of properties, seperated by commas.
///
/// # Examples:
/// Create an `Object`.
/// ```rust
/// use libspa::pod::{object, property};
///
/// let pod_object = object!{
///     libspa::utils::SpaTypes::ObjectParamFormat,
///     libspa::param::ParamType::EnumFormat,
///     property!(
///         libspa::format::FormatProperties::MediaType,
///         Id,
///         libspa::format::MediaType::Video
///     ),
///     property!(
///         libspa::format::FormatProperties::MediaSubtype,
///         Id,
///         libspa::format::MediaSubtype::Raw
///     ),
/// };
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __object__ {
    ($type_:expr, $id:expr, $($properties:expr),* $(,)?) => {
        pipewire::spa::pod::Object {
            type_: $type_.as_raw(),
            id: $id.as_raw(),
            properties: [ $( $properties, )* ].to_vec(),
        }
    };
}
#[doc(inline)]
pub use __object__ as object;

/// An object property.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// key of the property, list of valid keys depends on the objec type.
    pub key: u32,
    /// flags for the property.
    pub flags: PropertyFlags,
    /// value of the property.
    pub value: Value,
}

bitflags! {
    /// Property flags
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct PropertyFlags: u32 {
        // These flags are redefinitions from
        // https://gitlab.freedesktop.org/pipewire/pipewire/-/blob/master/spa/include/spa/pod/pod.h
        /// Property is read-only.
        const READONLY = spa_sys::SPA_POD_PROP_FLAG_READONLY;
        /// Property is some sort of hardware parameter.
        const HARDWARE = spa_sys::SPA_POD_PROP_FLAG_HARDWARE;
        /// Property contains a dictionary struct.
        const HINT_DICT = spa_sys::SPA_POD_PROP_FLAG_HINT_DICT;
        /// Property is mandatory.
        const MANDATORY = spa_sys::SPA_POD_PROP_FLAG_MANDATORY;
        /// Property choices need no fixation.
        #[cfg(feature = "v0_3_33")]
        const DONT_FIXATE = spa_sys::SPA_POD_PROP_FLAG_DONT_FIXATE;
    }
}

/// A macro for creating a new Object [`Property`].
///
/// The macro accepts the following:
/// - properties!(libspa::format::FormatProperties::`<key>`, Id, `<value>`)
/// - properties!(libspa::format::FormatProperties::`<key>`, `<type>`, libspa::utils::`<type>`(`<value>`))
/// - properties!(libspa::format::FormatProperties::`<key>`, Choice, Enum, Id, `<default>`, `<value>`, ...)
/// - properties!(libspa::format::FormatProperties::`<key>`, Choice, Enum, `<type>`,
///                 libspa::utils::`<type>`(`<default>`),
///                 libspa::utils::`<type>`(`<value>`), ...)
/// - properties!(libspa::format::FormatProperties::`<key>`, Choice, Flags, `<type>`,
///                 libspa::utils::`<type>`(`<default>`),
///                 libspa::utils::`<type>`(`<value>`), ...)
/// - properties!(libspa::format::FormatProperties::`<key>`, Choice, Step, `<type>`,
///                 libspa::utils::`<type>`(default),
///                 libspa::utils::`<type>`(min),
///                 libspa::utils::`<type>`(max),
///                 libspa::utils::`<type>`(step))
/// - properties!(libspa::format::FormatProperties::`<key>`, Choice, Range, `<type>`,
///                 libspa::utils::`<type>`(default),
///                 libspa::utils::`<type>`(min),
///                 libspa::utils::`<type>`(max))
#[doc(hidden)]
#[macro_export]
macro_rules! __property__ {
    ($key:expr, $value:expr) => {
        pipewire::spa::pod::Property {
            key: $key.as_raw(),
            flags: pipewire::spa::pod::PropertyFlags::empty(),
            value: $value,
        }
    };

    ($key:expr, Id, $value:expr) => {
        pipewire::spa::pod::property!($key, pipewire::spa::pod::Value::Id(pipewire::spa::utils::Id($value.as_raw())))
    };

    ($key:expr, $type_:ident, $value:expr) => {
        pipewire::spa::pod::property!($key, pipewire::spa::pod::Value::$type_($value))
    };

    ($key:expr, Choice, Enum, Id, $default:expr, $($alternative:expr),+ $(,)?) => {
        pipewire::spa::pod::property!(
            $key,
            pipewire::spa::pod::Value::Choice(pipewire::spa::pod::ChoiceValue::Id(
                pipewire::spa::utils::Choice::<pipewire::spa::utils::Id>(
                    pipewire::spa::utils::ChoiceFlags::empty(),
                    pipewire::spa::utils::ChoiceEnum::<pipewire::spa::utils::Id>::Enum {
                        default: pipewire::spa::utils::Id($default.as_raw()),
                        alternatives: [ $( pipewire::spa::utils::Id($alternative.as_raw()), )+ ].to_vec()
                    }
                )
            ))
        )
    };

    ($key:expr, Choice, Enum, $type_:ident, $default:expr, $($alternative:expr),+ $(,)?) => {
        pipewire::spa::pod::property!(
            $key,
            pipewire::spa::pod::Value::Choice(pipewire::spa::pod::ChoiceValue::$type_(
                pipewire::spa::utils::Choice::<pipewire::spa::utils::$type_>(
                    pipewire::spa::utils::ChoiceFlags::empty(),
                    pipewire::spa::utils::ChoiceEnum::<pipewire::spa::utils::$type_>::Enum {
                        default: $default,
                        alternatives: [ $( $alternative, )+ ].to_vec()
                    }
                )
            ))
        )
    };

    ($key:expr, Choice, Flags, $type_:ident, $default:expr, $($alternative:expr),+ $(,)?) => {
        pipewire::spa::pod::property!(
            $key,
            pipewire::spa::pod::Value::Choice(pipewire::spa::pod::ChoiceValue::$type_(
                pipewire::spa::utils::Choice::<pipewire::spa::utils::$type_>(
                    pipewire::spa::utils::ChoiceFlags::empty(),
                    pipewire::spa::utils::ChoiceEnum::<pipewire::spa::utils::$type_>::Flags {
                        default: $default,
                        flags: [ $( $alternative, )+ ].to_vec()
                    }
                )
            ))
        )
    };

    ($key:expr, Choice, Step, $type_:ident, $default:expr, $min:expr, $max:expr, $step:expr) => {
        pipewire::spa::pod::property!(
            $key,
            pipewire::spa::pod::Value::Choice(pipewire::spa::pod::ChoiceValue::$type_(
                pipewire::spa::utils::Choice::<pipewire::spa::utils::$type_>(
                    pipewire::spa::utils::ChoiceFlags::empty(),
                    pipewire::spa::utils::ChoiceEnum::<pipewire::spa::utils::$type_>::Step {
                        default: $default,
                        min: $min,
                        max: $max,
                        step: $step,
                    }
                )
            ))
        )
    };

    ($key:expr, Choice, Range, $type_:ident, $default:expr, $min:expr, $max:expr) => {
        pipewire::spa::pod::property!(
            $key,
            pipewire::spa::pod::Value::Choice(pipewire::spa::pod::ChoiceValue::$type_(
                pipewire::spa::utils::Choice::<pipewire::spa::utils::$type_>(
                    pipewire::spa::utils::ChoiceFlags::empty(),
                    pipewire::spa::utils::ChoiceEnum::<pipewire::spa::utils::$type_>::Range {
                        default: $default,
                        min: $min,
                        max: $max,
                    }
                )
            ))
        )
    };
}
#[doc(inline)]
pub use __property__ as property;

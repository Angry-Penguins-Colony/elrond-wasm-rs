use alloc::vec::Vec;
use alloc::boxed::Box;
use core::num::NonZeroUsize;

use crate::top_de_input::TopDecodeInput;
use crate::nested_de::*;
use crate::TypeInfo;
use crate::codec_err::DecodeError;
use crate::transmute::*;

/// Trait that allows zero-copy read of values from an underlying API in big endian format.
/// 
/// 'Top' stands for the fact that values are deserialized on their own,
/// so we have the benefit of knowing their length.
/// This is useful in many scnearios, such as not having to encode Vec length and others.
/// 
/// The opther optimization that can be done when deserializing top-level objects
/// is using special functions from the underlying API that do some of the work for the deserializer.
/// These include getting values directly as i64/u64 or wrapping them directly into an owned Box<[u8]>.
/// 
/// BigInt/BigUint handling is not included here, because these are API-dependent
/// and would overly complicate the trait.
/// 
pub trait TopDecode: Sized {
    #[doc(hidden)]
    const TYPE_INFO: TypeInfo = TypeInfo::Unknown;
    
    /// Attempt to deserialize the value from input.
    fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R;
    
    /// Allows types to provide optimized implementations for their boxed version.
    /// Especially designed for byte arrays that can be transmuted directly from the input sometimes.
    #[doc(hidden)]
    #[inline]
    fn top_decode_boxed<I: TopDecodeInput, R, F: FnOnce(Result<Box<Self>, DecodeError>) -> R>(input: I, f: F) -> R {
        Self::top_decode(input, |res| match res {
            Ok(v) => f(Ok(Box::new(v))),
            Err(e) => f(Err(e))
        })
    }
}

pub fn top_decode_from_nested<D, I, R, F>(input: I, f: F) -> R
where
    I: TopDecodeInput,
    D: NestedDecode,
    F: FnOnce(Result<D, DecodeError>) -> R,
{
    let bytes = input.into_boxed_slice_u8();
    let mut_slice = &mut &*bytes;
    match D::dep_decode_to(mut_slice) {
        Ok(result) => {
            if mut_slice.is_empty() {
                f(Ok(result))
            } else {
                f(Err(DecodeError::INPUT_TOO_LONG))
            }
        },
        Err(e) => f(Err(e)),
    }
}

impl TopDecode for () {
    const TYPE_INFO: TypeInfo = TypeInfo::Unit;
    
	fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(_input: I, f: F) -> R {
		f(Ok(()))
	}
}

impl<T: TopDecode> TopDecode for Box<T> {
	fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
        T::top_decode_boxed(input, f)
    }
}

// Allowed to implement this because [T] cannot implement NestedDecode, being ?Sized.
impl<T: NestedDecode> TopDecode for Box<[T]> {
	fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
        if let TypeInfo::U8 = T::TYPE_INFO {
            let bytes = input.into_boxed_slice_u8();
            let cast_bytes: Box<[T]> = unsafe { core::mem::transmute(bytes) };
            f(Ok(cast_bytes))
        } else {
            Vec::<T>::top_decode(input, |res| match res {
                Ok(vec) => f(Ok(vec_into_boxed_slice(vec))),
                Err(e) => f(Err(e)),
            })
        }
    }
}

impl<T: NestedDecode> TopDecode for Vec<T> {
	fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
        if let TypeInfo::U8 = T::TYPE_INFO {
            let bytes = input.into_boxed_slice_u8();
            let bytes_vec = boxed_slice_into_vec(bytes);
            let cast_vec: Vec<T> = unsafe { core::mem::transmute(bytes_vec) };
            f(Ok(cast_vec))
        } else {
            let bytes = input.into_boxed_slice_u8();
            // let mut slice = input.get_slice_u8();
            let mut_slice = &mut &*bytes;
            let mut result: Vec<T> = Vec::new();
            while !mut_slice.is_empty() {
                match T::dep_decode_to(mut_slice) {
                    Ok(t) => { result.push(t); }
                    Err(e) => { return f(Err(e)); }
                }
            }
            f(Ok(result))
        }
    }
}

macro_rules! decode_num_unsigned {
    ($ty:ty, $bounds_ty:ty, $type_info:expr) => {
        impl TopDecode for $ty {
            const TYPE_INFO: TypeInfo = $type_info;
            
            fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
                let arg_u64 = input.into_u64();
                let max = <$bounds_ty>::MAX as u64;
                if arg_u64 > max {
                    f(Err(DecodeError::INPUT_TOO_LONG))
                } else {
                    f(Ok(arg_u64 as $ty))
                }
            }
        }
    }
}

decode_num_unsigned!(u8, u8, TypeInfo::U8);
decode_num_unsigned!(u16, u16, TypeInfo::U16);
decode_num_unsigned!(u32, u32, TypeInfo::U32);
decode_num_unsigned!(usize, u32, TypeInfo::USIZE); // even if usize can be 64 bits on some platforms, we always deserialize as max 32 bits
decode_num_unsigned!(u64, u64, TypeInfo::U64);

macro_rules! decode_num_signed {
    ($ty:ty, $bounds_ty:ty, $type_info:expr) => {
        impl TopDecode for $ty {
            const TYPE_INFO: TypeInfo = $type_info;
            
            fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
                let arg_i64 = input.into_i64();
                let min = <$bounds_ty>::MIN as i64;
                let max = <$bounds_ty>::MAX as i64;
                if arg_i64 < min || arg_i64 > max {
                    f(Err(DecodeError::INPUT_OUT_OF_RANGE))
                } else {
                    f(Ok(arg_i64 as $ty))
                }
            }
        }
    }
}

decode_num_signed!(i8 , i8, TypeInfo::I8);
decode_num_signed!(i16, i16, TypeInfo::I16);
decode_num_signed!(i32, i32, TypeInfo::I32);
decode_num_signed!(isize, i32, TypeInfo::ISIZE); // even if isize can be 64 bits on some platforms, we always deserialize as max 32 bits
decode_num_signed!(i64, i64, TypeInfo::I64);

impl TopDecode for bool {
    const TYPE_INFO: TypeInfo = TypeInfo::Bool;
    
	fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
        match input.into_u64() {
            0 => f(Ok(false)),
            1 => f(Ok(true)),
            _ => f(Err(DecodeError::INPUT_OUT_OF_RANGE)),
        }
    }
}

impl<T: NestedDecode> TopDecode for Option<T> {
	fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
        let bytes = input.into_boxed_slice_u8();
        if bytes.is_empty() {
            f(Ok(None))
        } else {
            match dep_decode_from_byte_slice::<T>(&bytes[1..]) {
                Ok(t) => f(Ok(Some(t))),
                Err(e) => f(Err(e)),
            }
            
        }
    }
}

macro_rules! tuple_impls {
    ($($len:expr => ($($n:tt $name:ident)+))+) => {
        $(
            impl<$($name),+> TopDecode for ($($name,)+)
            where
                $($name: NestedDecode,)+
            {
                fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
                    top_decode_from_nested(input, f)
				}
            }
        )+
    }
}

tuple_impls! {
    1 => (0 T0)
    2 => (0 T0 1 T1)
    3 => (0 T0 1 T1 2 T2)
    4 => (0 T0 1 T1 2 T2 3 T3)
    5 => (0 T0 1 T1 2 T2 3 T3 4 T4)
    6 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5)
    7 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6)
    8 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7)
    9 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8)
    10 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9)
    11 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10)
    12 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11)
    13 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12)
    14 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13)
    15 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14)
    16 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15)
}

macro_rules! array_impls {
    ($($n: tt,)+) => {
        $(
            impl<T: NestedDecode> TopDecode for [T; $n] {
                fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
                    top_decode_from_nested(input, f)
                }
                
                fn top_decode_boxed<I: TopDecodeInput, R, F: FnOnce(Result<Box<Self>, DecodeError>) -> R>(input: I, f: F) -> R {
                    if let TypeInfo::U8 = T::TYPE_INFO {
                        // transmute directly
                        let bs = input.into_boxed_slice_u8();
                        if bs.len() != $n {
                            return f(Err(DecodeError::ARRAY_DECODE_ERROR));
                        }
                        let raw = Box::into_raw(bs);
                        let array_box = unsafe { Box::<[T; $n]>::from_raw(raw as *mut [T; $n]) };
                        f(Ok(array_box))
                    } else {
                        Self::top_decode(input, |res| match res {
                            Ok(v) => f(Ok(Box::new(v))),
                            Err(e) => f(Err(e))
                        })
                    }
                }
            }
        )+
    }
}

array_impls!(
	1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
	17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
	32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51,
	52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71,
	72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91,
	92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108,
	109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124,
	125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140,
	141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156,
	157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172,
	173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188,
	189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204,
	205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220,
	221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236,
	237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252,
	253, 254, 255, 256, 384, 512, 768, 1024, 2048, 4096, 8192, 16384, 32768,
);

fn decode_non_zero_usize(num: usize) -> Result<NonZeroUsize, DecodeError> {
    if let Some(nz) = NonZeroUsize::new(num) {
        Ok(nz)
    } else {
        Err(DecodeError::INVALID_VALUE)
    }
}

impl TopDecode for NonZeroUsize {
    fn top_decode<I: TopDecodeInput, R, F: FnOnce(Result<Self, DecodeError>) -> R>(input: I, f: F) -> R {
        usize::top_decode(input, |res| match res {
            Ok(num) => f(decode_non_zero_usize(num)),
            Err(e) => f(Err(e))
        })
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
	use super::super::test_struct::*;
    use core::fmt::Debug;

    fn deser_ok<V>(element: V, bytes: &[u8])
    where
        V: TopDecode + PartialEq + Debug + 'static,
    {
        let deserialized: V = V::top_decode(bytes, |res| res.unwrap());
        assert_eq!(deserialized, element);
    }

    #[test]
    fn test_top_numbers_decompacted() {
        // unsigned positive
        deser_ok(5u8, &[5]);
        deser_ok(5u16, &[5]);
        deser_ok(5u32, &[5]);
        deser_ok(5u64, &[5]);
        deser_ok(5usize, &[5]);
        // signed positive
        deser_ok(5i8, &[5]);
        deser_ok(5i16, &[5]);
        deser_ok(5i32, &[5]);
        deser_ok(5i64, &[5]);
        deser_ok(5isize, &[5]);
        // signed negative
        deser_ok(-5i8, &[251]);
        deser_ok(-5i16, &[251]);
        deser_ok(-5i32, &[251]);
        deser_ok(-5i64, &[251]);
        deser_ok(-5isize, &[251]);
        // non zero usize
        deser_ok(NonZeroUsize::new(5).unwrap(), &[5]);
    }

    #[test]
    fn test_top_numbers_decompacted_2() {
        deser_ok(-1i32, &[255]);
        deser_ok(-1i32, &[255, 255]);
        deser_ok(-1i32, &[255, 255, 255, 255]);
        deser_ok(-1i64, &[255, 255, 255, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn test_struct() {
        let test = Test {
            int: 1,
            seq: [5, 6].to_vec(),
            another_byte: 7,
        };
        deser_ok(test, &[0, 1, 0, 0, 0, 2, 5, 6, 7]);
    }

    #[test]
    fn test_enum() {
        let u = E::Unit;
        let expected: &[u8] = &[/*variant index*/ 0, 0, 0, 0];
        deser_ok(u, expected);

        let n = E::Newtype(1);
        let expected: &[u8] = &[/*variant index*/ 0, 0, 0, 1, /*data*/ 0, 0, 0, 1];
        deser_ok(n, expected);

        let t = E::Tuple(1, 2);
        let expected: &[u8] = &[/*variant index*/ 0, 0, 0, 2, /*(*/ 0, 0, 0, 1, /*,*/ 0, 0, 0, 2 /*)*/];
        deser_ok(t, expected);

        let s = E::Struct { a: 1 };
        let expected: &[u8] = &[/*variant index*/ 0, 0, 0, 3, /*data*/ 0, 0, 0, 1];
        deser_ok(s, expected);
    }
}

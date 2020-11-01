use crate::*;
use crate::elrond_codec::*;
use core::marker::PhantomData;

struct ApiOutput<A, BigInt, BigUint>
where
    BigUint: BigUintApi + 'static,
    BigInt: BigIntApi<BigUint> + 'static,
    A: ContractIOApi<BigInt, BigUint> + 'static 
{
    api: A,
    _phantom1: PhantomData<BigInt>,
    _phantom2: PhantomData<BigUint>,
}

impl<A, BigInt, BigUint> ApiOutput<A, BigInt, BigUint>
where
    BigUint: BigUintApi + 'static,
    BigInt: BigIntApi<BigUint> + 'static,
    A: ContractIOApi<BigInt, BigUint> + 'static 
{
    #[inline]
    fn new(api: A) -> Self {
        ApiOutput {
            api,
            _phantom1: PhantomData,
            _phantom2: PhantomData,
        }
    }
}

impl<A, BigInt, BigUint> TopEncodeOutput for ApiOutput<A, BigInt, BigUint>
where
    BigUint: BigUintApi + 'static,
    BigInt: BigIntApi<BigUint> + 'static,
    A: ContractIOApi<BigInt, BigUint> + 'static 
{
    fn set_slice_u8(self, bytes: &[u8]) {
        self.api.finish_slice_u8(bytes);
    }

    fn set_u64(self, value: u64) {
        self.api.finish_u64(value);
    }

    fn set_i64(self, value: i64) {
        self.api.finish_i64(value);
    }

    #[inline]
    fn set_unit(self) {
        // nothing: no result produced
    }

    #[inline]
    fn set_big_int_handle_or_bytes<F: FnOnce() -> Vec<u8>>(self, handle: i32, _else_bytes: F) {
        self.api.finish_big_int_raw(handle);
    }

    #[inline]
    fn set_big_uint_handle_or_bytes<F: FnOnce() -> Vec<u8>>(self, handle: i32, _else_bytes: F) {
        self.api.finish_big_uint_raw(handle);
    }
    
}

pub trait EndpointResult<A, BigInt, BigUint>: Sized
where
    BigUint: BigUintApi + 'static,
    BigInt: BigIntApi<BigUint> + 'static,
    A: ContractHookApi<BigInt, BigUint> + ContractIOApi<BigInt, BigUint> + 'static 
{
    fn finish(&self, api: A);
}

/// All serializable objects can be used as smart contract function result.
impl<A, BigInt, BigUint, T> EndpointResult<A, BigInt, BigUint> for T
where
    T: TopEncode,
    BigUint: BigUintApi + 'static,
    BigInt: BigIntApi<BigUint> + 'static,
    A: ContractHookApi<BigInt, BigUint> + ContractIOApi<BigInt, BigUint> + 'static
{
    fn finish(&self, api: A) {
        let res = self.top_encode(ApiOutput::new(api.clone()));
        if let Err(encode_err_message) = res {
            api.signal_error(encode_err_message.message_bytes());
        }
    }
}

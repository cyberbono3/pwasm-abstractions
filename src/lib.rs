#![no_std]
#![allow(non_snake_case)]
#![feature(proc_macro)]
#![feature(proc_macro_gen)]
#![feature(alloc)]
#![feature(iterator_flatten)]

#[macro_use]
extern crate alloc;
/// Bigint used for 256-bit arithmetic
extern crate bigint;
extern crate parity_hash;
extern crate pwasm_abi;
extern crate pwasm_abi_derive;
extern crate pwasm_ethereum;
extern crate pwasm_std;

pub mod utils {
    use bigint::U256;
    use core::marker::PhantomData;
    use core::mem;
    use core::ptr;
    use parity_hash::H256;
    use pwasm_ethereum;
    use pwasm_std::Vec;

    pub struct SubAddress {
        address: H256,
        offset: usize,
    }

    impl SubAddress {
        pub fn new(address: H256, offset: usize) -> Self {
            assert!(offset < 32);
            Self { address, offset }
        }

        pub fn address(&self) -> H256 {
            self.address
        }

        pub fn offset(&self) -> usize {
            self.offset
        }

        pub fn get_sub_address(&self, index: u32) -> SubAddress {
            let bytes: [u8; 4] = unsafe { mem::transmute(index) };
            let address_bytes = self.address.0;
            let mut address = [0u8; 32];
            address[..self.offset].copy_from_slice(&address_bytes[..self.offset]);
            address[self.offset..self.offset + bytes.len()].copy_from_slice(&bytes);
            Self {
                address: H256(address),
                offset: self.offset + bytes.len(),
            }
        }
    }
}

pub mod collections {
    use bigint::U256;
    use core::marker::PhantomData;
    use core::mem;
    use core::ptr;
    use parity_hash::H256;
    use pwasm_ethereum;
    use pwasm_std::Vec;
    use utils::SubAddress;

    pub trait Serialize {
        fn from_bytes(bytes: &[u8]) -> Self;
        fn to_bytes(&self) -> Vec<u8>;
    }

    pub struct Array<T: Serialize> {
        pub address: SubAddress,
        marker_: PhantomData<T>,
    }

    impl<T: Serialize> Array<T> {
        pub fn new(address: SubAddress) -> Self {
            Self {
                address,
                marker_: PhantomData,
            }
        }

        pub fn len(&self) -> U256 {
            pwasm_ethereum::read(&self.address.address()).into()
        }

        pub fn get_item(&self, index: U256) -> Option<T> {
            if index >= self.len() {
                return None;
            }

            let size_in_buckets = Self::get_size_in_buckets();
            let self_address: U256 = self.address.address().into();
            let base_address: U256 = self_address + index * size_in_buckets.into() + 1.into();

            let mut bytes = Vec::with_capacity(mem::size_of::<T>());

            for i in 0..size_in_buckets {
                let chunk = pwasm_ethereum::read(&(base_address + i.into()).into());
                bytes.extend_from_slice(&chunk);
            }

            Some(T::from_bytes(&bytes))
        }

        pub fn push(&mut self, value: T) {
            let len: U256 = self.len();
            let bytes = value.to_bytes();
            let index = len;

            let size_in_buckets = Self::get_size_in_buckets();
            let self_address: U256 = self.address.address().into();
            let base_address: U256 = self_address + index * size_in_buckets.into() + 1.into();
            let mut arr = [0u8; 32];

            for (i, chunk) in bytes.chunks(32).enumerate() {
                let chunk = if chunk.len() == 32 {
                    unsafe { mem::transmute(chunk.as_ptr()) }
                } else {
                    arr[..chunk.len()].copy_from_slice(chunk);
                    &arr
                };

                pwasm_ethereum::write(&(base_address + i.into()).into(), chunk);
            }

            pwasm_ethereum::write(&self.address.address(), &(len + 1.into()).into());
        }

        fn get_size_in_buckets() -> usize {
            let buckets = mem::size_of::<T>() / 32;
            if buckets * 32 < mem::size_of::<T>() {
                buckets + 1
            } else {
                buckets
            }
        }
    }

    impl<'a, T: Serialize> IntoIterator for &'a Array<T> {
        type Item = T;
        type IntoIter = ArrayIterator<'a, T>;

        fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
            ArrayIterator {
                array: self,
                index: 0.into(),
            }
        }
    }

    pub struct ArrayIterator<'a, T: 'a + Serialize> {
        array: &'a Array<T>,
        index: U256,
    }

    impl<'a, T: Serialize> Iterator for ArrayIterator<'a, T> {
        type Item = T;

        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            let result = self.array.get_item(self.index);
            if result.is_some() {
                self.index = self.index + 1.into();
            }
            result
        }
    }

    impl Serialize for u32 {
        fn from_bytes(bytes: &[u8]) -> Self {
            unsafe { ptr::read(bytes.as_ptr() as *const _) }
        }

        fn to_bytes(&self) -> Vec<u8> {
            let result: [u8; 4] = unsafe { mem::transmute_copy(self) };
            vec![result[0], result[1], result[2], result[3]]
        }
    }

    impl Serialize for usize {
        fn from_bytes(bytes: &[u8]) -> Self {
            unsafe { ptr::read(bytes.as_ptr() as *const _) }
        }

        fn to_bytes(&self) -> Vec<u8> {
            let result: [u8; 4] = unsafe { mem::transmute_copy(self) };
            vec![result[0], result[1], result[2], result[3]]
        }
    }

    impl Serialize for U256 {
        fn from_bytes(bytes: &[u8]) -> Self {
            unsafe { ptr::read(bytes.as_ptr() as *const _) }
        }

        fn to_bytes(&self) -> Vec<u8> {
            let result: [u8; 32] = unsafe { mem::transmute_copy(self) };
            result.into_iter().map(|x| *x).collect()
        }
    }

    impl<T: Serialize> Serialize for Array<T> {
        fn from_bytes(bytes: &[u8]) -> Self {
            unsafe { ptr::read(bytes.as_ptr() as *const _) }
        }

        fn to_bytes(&self) -> Vec<u8> {
            let result: [u8; mem::size_of::<SubAddress>()] =
                unsafe { mem::transmute_copy(&self.address) };
            result.into_iter().map(|x| *x).collect()
        }
    }
}

pub mod contract {
    use bigint::U256;
    use collections::*;
    use parity_hash::H256;
    use pwasm_abi_derive::eth_abi;
    use pwasm_ethereum;
    use pwasm_std::Vec;
    use utils::SubAddress;

    macro_rules! storage_keys {
        () => {};
        ($($name:ident),*) => {
            storage_keys!(0u8, $($name),*);
        };
        ($count:expr, $name:ident) => {
            static $name: H256 = H256([$count, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]);
        };
        ($count:expr, $name:ident, $($tail:ident),*) => {
            static $name: H256 = H256([$count, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]);
            storage_keys!($count + 1u8, $($tail),*);
        };
    }

    #[eth_abi(TokenEndpoint, TokenClient)]
    pub trait SampleContractInterface {
        /// The constructor
        fn constructor(&mut self, _total_supply: U256);
        /// Total amount of tokens
        #[constant]
        fn totalSupply(&mut self) -> U256;

        #[constant]
        fn getNumbers(&mut self) -> Vec<U256>;

        #[constant]
        fn getNumbersCount(&mut self) -> U256;

        fn addNumber(&mut self, value: U256);
    }

    storage_keys!(TOTAL_SUPPLY_KEY, DATA_KEY);

    pub struct SampleContract;

    impl SampleContractInterface for SampleContract {
        fn constructor(&mut self, total_supply: U256) {
            // Set up the total supply for the token
            pwasm_ethereum::write(&TOTAL_SUPPLY_KEY, &total_supply.into());
        }

        fn totalSupply(&mut self) -> U256 {
            pwasm_ethereum::read(&TOTAL_SUPPLY_KEY).into()
        }

        fn getNumbers(&mut self) -> Vec<U256> {
            let array: Array<U256> = Array::new(SubAddress::new(DATA_KEY.clone(), 0));
            array.into_iter().collect()
        }

        fn getNumbersCount(&mut self) -> U256 {
            let array: Array<U256> = Array::new(SubAddress::new(DATA_KEY.clone(), 0));
            array.len()
        }

        fn addNumber(&mut self, value: U256) {
            let mut array: Array<U256> = Array::new(SubAddress::new(DATA_KEY.clone(), 0));
            array.push(value);
        }
    }
}
// Declares the dispatch and dispatch_ctor methods
use contract::*;
use pwasm_abi::eth::EndpointInterface;

#[no_mangle]
pub fn call() {
    let mut endpoint = TokenEndpoint::new(SampleContract {});
    // Read http://solidity.readthedocs.io/en/develop/abi-spec.html#formal-specification-of-the-encoding for details
    pwasm_ethereum::ret(&endpoint.dispatch(&pwasm_ethereum::input()));
}

#[no_mangle]
pub fn deploy() {
    let mut endpoint = TokenEndpoint::new(SampleContract {});
    endpoint.dispatch_ctor(&pwasm_ethereum::input());
}

#[cfg(test)]
mod tests {
    extern crate pwasm_test;
    extern crate std;
    use self::pwasm_test::ext_reset;
    use bigint::U256;
    use collections::*;
    use contract::*;
    use parity_hash::{Address, H256};
    use pwasm_std::Vec;
    use utils::*;

    #[test]
    fn should_push_and_get_items_from_array() {
        let array_address = 0.into();
        let mut array: Array<u32> = Array::new(SubAddress::new(array_address, 0));
        array.push(123);
        array.push(456);

        let len = array.len();
        let first_item = array.get_item(0.into());
        let second_item = array.get_item(1.into());

        let expected_len: U256 = 2.into();
        assert_eq!(expected_len, len);
        assert_eq!(Some(123), first_item);
        assert_eq!(Some(456), second_item);
    }

    #[test]
    fn should_push_and_get_items_from_array_256() {
        let array_address = 0.into();
        let mut array: Array<U256> = Array::new(SubAddress::new(array_address, 0));
        array.push(123.into());
        array.push(456.into());

        let len = array.len();
        let first_item = array.get_item(0.into());
        let second_item = array.get_item(1.into());

        let expected_len: U256 = 2.into();
        assert_eq!(expected_len, len);
        assert_eq!(Some(123.into()), first_item);
        assert_eq!(Some(456.into()), second_item);
    }

    #[test]
    fn should_work() {
        let mut contract = SampleContract {};
        let owner_address: Address = "0xea674fdde714fd979de3edf0f56aa9716b898ec8".into();
        ext_reset(|e| e.sender(owner_address.clone()));
        let total_supply = 10000.into();
        let first_num = 123.into();
        let second_num = 456.into();
        contract.constructor(total_supply);
        contract.addNumber(first_num);
        contract.addNumber(second_num);

        let len = contract.getNumbersCount();
        let items = contract.getNumbers();

        let expected_len: U256 = 2.into();
        assert_eq!(total_supply, contract.totalSupply());
        assert_eq!(expected_len, len);
        assert_eq!(first_num, items[0]);
        assert_eq!(second_num, items[1]);
    }

    #[test]
    fn should_create_sub_address() {
        let address = SubAddress::new(H256([0u8; 32]), 0);
        let sub_address = address.get_sub_address(1 + 256);
        let sub_sub_address =
            sub_address.get_sub_address(1 + 2 * 256 + 3 * 256 * 256 + 4 * 256 * 256 * 256);

        assert_eq!(4, sub_address.offset());
        assert_eq!(8, sub_sub_address.offset());
        assert_eq!(
            H256([
                1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0
            ]),
            sub_address.address()
        );
        assert_eq!(
            H256([
                1, 1, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0
            ]),
            sub_sub_address.address()
        );
    }

    #[test]
    fn should_push_to_nested_array() {
        let array_address = 0.into();
        let mut array: Array<Array<U256>> = Array::new(SubAddress::new(array_address, 0));
        let mut sub_array: Array<U256> = Array::new(array.address.get_sub_address(0));

        sub_array.push(123.into());
        sub_array.push(456.into());
        array.push(sub_array);

        let mut sub_array: Array<U256> = Array::new(array.address.get_sub_address(1));
        sub_array.push(789.into());
        array.push(sub_array);

        let len = array.len();
        let items: Vec<U256> = array
            .into_iter()
            .map(|x| x.into_iter().collect::<Vec<_>>())
            .flatten()
            .collect();

        let expected_len: U256 = 2.into();
        assert_eq!(expected_len, len);
        assert_eq!(3, items.len());
    }
}

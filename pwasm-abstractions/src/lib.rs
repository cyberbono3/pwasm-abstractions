#![no_std]
#![feature(alloc)]

#[macro_use]
extern crate alloc;
/// Bigint used for 256-bit arithmetic
extern crate bigint;
extern crate parity_hash;
extern crate pwasm_ethereum;
extern crate pwasm_std;

pub mod utils {
    use core::mem;
    use parity_hash::H256;

    #[derive(Copy, Clone)]
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
            let bytes: [u8; 4] = unsafe { mem::transmute(index + 1) };
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

    pub trait Serialize: Copy {
        fn from_bytes(bytes: &[u8]) -> Self;
        fn to_bytes(&self) -> Vec<u8>;
    }

    #[derive(Copy, Clone)]
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

        pub fn len(&self) -> u32 {
            let raw: U256 = pwasm_ethereum::read(&self.len_address()).into();
            raw.as_u32()
        }

        pub fn get_item(&self, index: u32) -> Option<T> {
            if index >= self.len() {
                return None;
            }

            let size_in_buckets = Self::get_size_in_buckets();
            let actual_index = size_in_buckets * index + 1;

            let mut bytes = Vec::with_capacity(mem::size_of::<T>());

            for i in 0..size_in_buckets {
                let address = self.address.get_sub_address(actual_index + i).address();
                let chunk = pwasm_ethereum::read(&address);
                bytes.extend_from_slice(&chunk);
            }

            Some(T::from_bytes(&bytes))
        }

        pub fn push(&mut self, value: T) {
            let len = self.len();
            let bytes = value.to_bytes();
            let index = len;

            let size_in_buckets = Self::get_size_in_buckets();
            let actual_index = size_in_buckets * index + 1;
            let mut arr = [0u8; 32];

            let mut counter: u32 = 0;

            for (i, chunk) in bytes.chunks(32).map(|a| {
                let ret = (counter, a);
                // Possible undefined overflow.
                counter += 1;
                ret
            }) {
                let chunk = if chunk.len() == 32 {
                    unsafe { mem::transmute(chunk.as_ptr()) }
                } else {
                    arr[..chunk.len()].copy_from_slice(chunk);
                    &arr
                };
                let address = self.address.get_sub_address(actual_index + i).address();

                pwasm_ethereum::write(&address, chunk);
            }

            let new_len: U256 = (len + 1).into();
            let len_address = self.len_address();
            pwasm_ethereum::write(&len_address, &new_len.into());
        }

        fn len_address(&self) -> H256 {
            self.address.get_sub_address(0).address()
        }

        fn get_size_in_buckets() -> u32 {
            let buckets = mem::size_of::<T>() / 32;
            let result = if buckets * 32 < mem::size_of::<T>() {
                buckets + 1
            } else {
                buckets
            };
            result as u32
        }
    }

    impl<'a, T: Serialize> IntoIterator for &'a Array<T> {
        type Item = T;
        type IntoIter = ArrayIterator<'a, T>;

        fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
            ArrayIterator {
                array: self,
                index: 0,
            }
        }
    }

    pub struct ArrayIterator<'a, T: 'a + Serialize> {
        array: &'a Array<T>,
        index: u32,
    }

    impl<'a, T: Serialize> Iterator for ArrayIterator<'a, T> {
        type Item = T;

        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            let result = self.array.get_item(self.index);
            if result.is_some() {
                self.index += 1;
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

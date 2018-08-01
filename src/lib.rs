#![no_std]
#![allow(non_snake_case)]
#![feature(proc_macro_gen)]
#![feature(use_extern_macros)]
#![feature(alloc)]

extern crate bigint;
extern crate parity_hash;
extern crate pwasm_abi;
extern crate pwasm_abi_derive;
extern crate pwasm_abstractions;
extern crate pwasm_ethereum;
extern crate pwasm_std;

pub mod contract {
    use core::mem;
    use core::ptr;
    use parity_hash::H256;
    use pwasm_abstractions::collections::*;
    use pwasm_abstractions::utils::*;
    use pwasm_ethereum;
    use pwasm_std::Vec;
    use pwasm_abi_derive::eth_abi;

    macro_rules! impl_field {
        ($count:expr, $name:ident) => {
            struct $name;

            #[allow(unused)]
            impl $name {
                const ADDRESS: H256 = H256([
                    $count, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0,
                ]);

                fn get_value() -> [u8; 32] {
                    pwasm_ethereum::read(&$name::ADDRESS)
                }

                fn set_value(value: &[u8; 32]) {
                    pwasm_ethereum::write(&$name::ADDRESS, value);
                }

                fn as_array<T: Serialize>() -> Array<T> {
                    Array::new(SubAddress::new($name::ADDRESS.clone(), 0))
                }
            }
        };
    }

    macro_rules! make_storage {
        () => {};
        ($($name:ident),*) => {
            make_storage!(0u8, $($name),*);
        };
        ($count:expr, $name:ident) => {
            impl_field!($count, $name);
        };
        ($count:expr, $name:ident, $($tail:ident),*) => {
            impl_field!($count, $name);
            make_storage!($count + 1u8, $($tail),*);
        };
    }

    #[eth_abi(TokenEndpoint, TokenClient)]
    pub trait SampleContractInterface {
        fn constructor(&mut self);
        fn createRequest(&mut self, serviceNumber: [u8; 30], date: u64, declarantType: u32);
        #[constant]
        fn getRequestByIndex(&mut self, index: u32) -> ([u8; 30], u64, u32);
        #[constant]
        fn getRequestsCount(&mut self) -> u32;
    }

    make_storage!(Requests);

    pub struct SampleContract;

    impl SampleContractInterface for SampleContract {
        fn constructor(&mut self) {}

        fn createRequest(&mut self, serviceNumber: [u8; 30], date: u64, declarantType: u32) {
            let mut array: Array<Request> = Requests::as_array();
            let request = Request {
                service_number: serviceNumber,
                date,
                declarant_type: declarantType,
            };

            array.push(request);
        }

        fn getRequestByIndex(&mut self, index: u32) -> ([u8; 30], u64, u32) {
            let array: Array<Request> = Requests::as_array();
            let item = array.get_item(index);
            item.map(|x| (x.service_number, x.date, x.declarant_type))
                .unwrap_or(([0; 30], 0, 0))
        }

        fn getRequestsCount(&mut self) -> u32 {
            let array: Array<Request> = Requests::as_array();
            array.len()
        }
    }

    #[derive(Debug, Copy, Clone)]
    struct Request {
        service_number: [u8; 30],
        date: u64,
        declarant_type: u32,
    }

    impl Serialize for Request {
        fn from_bytes(bytes: &[u8]) -> Self {
            let service_number = &bytes[0..30];
            let date = &bytes[30..38];
            let declarant_type = &bytes[38..];
            Self {
                service_number: unsafe { ptr::read(service_number.as_ptr() as *const _) },
                date: unsafe { ptr::read(date.as_ptr() as *const _) },
                declarant_type: unsafe { ptr::read(declarant_type.as_ptr() as *const _) },
            }
        }

        fn to_bytes(&self) -> Vec<u8> {
            let date: [u8; 8] = unsafe { mem::transmute_copy(&self.date) };
            let declarant_type: [u8; 4] = unsafe { mem::transmute_copy(&self.declarant_type) };
            let mut result = Vec::with_capacity(30 + 8 + 4);
            result.extend_from_slice(&self.service_number);
            result.extend_from_slice(&date);
            result.extend_from_slice(&declarant_type);
            result
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

    use self::pwasm_test::ext_reset;
    use contract::*;
    use parity_hash::Address;

    #[test]
    fn should_work() {
        let mut contract = SampleContract {};
        let owner_address: Address = "0xea674fdde714fd979de3edf0f56aa9716b898ec8".into();
        ext_reset(|e| e.sender(owner_address.clone()));
        contract.constructor();

        contract.createRequest(*b"123456789012345678901234567890", 10, 50);
        contract.createRequest(*b"523456789012345678901234567890", 20, 60);

        let len = contract.getRequestsCount();

        assert_eq!(2, len);
        assert_eq!(
            (*b"123456789012345678901234567890", 10, 50),
            contract.getRequestByIndex(0)
        );
        assert_eq!(
            (*b"523456789012345678901234567890", 20, 60),
            contract.getRequestByIndex(1)
        );
    }
}

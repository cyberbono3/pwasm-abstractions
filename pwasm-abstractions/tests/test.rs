#![feature(iterator_flatten)]

extern crate bigint;
extern crate parity_hash;
extern crate pwasm_abstractions;
extern crate pwasm_std;
extern crate pwasm_test;

use bigint::U256;
use parity_hash::H256;
use pwasm_abstractions::collections::*;
use pwasm_abstractions::utils::*;
use pwasm_std::Vec;

#[test]
fn should_push_and_get_items_from_array() {
    let array_address = 0.into();
    let mut array: Array<u32> = Array::new(SubAddress::new(array_address, 0));
    array.push(123);
    array.push(456);

    let len = array.len();
    let first_item = array.get_item(0);
    let second_item = array.get_item(1);

    assert_eq!(2, len);
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
    let first_item = array.get_item(0);
    let second_item = array.get_item(1);

    assert_eq!(2, len);
    assert_eq!(Some(123.into()), first_item);
    assert_eq!(Some(456.into()), second_item);
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
            2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0
        ]),
        sub_address.address()
    );
    assert_eq!(
        H256([
            2, 1, 0, 0, 2, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0
        ]),
        sub_sub_address.address()
    );
}

#[test]
fn should_push_to_nested_array() {
    let root_address = SubAddress::new(0.into(), 0);
    let array_address = root_address.get_sub_address(0);
    let subarray_address = array_address.get_sub_address(0);

    let mut array = Array::new(array_address);
    let mut sub_array = Array::new(subarray_address);

    sub_array.push(123);
    sub_array.push(456);
    array.push(sub_array);

    let mut sub_array = Array::new(array.address.get_sub_address(1));
    sub_array.push(789);
    array.push(sub_array);

    let len = array.len();
    let items: Vec<_> = array
        .into_iter()
        .map(|x| x.into_iter().collect::<Vec<_>>())
        .flatten()
        .collect();

    assert_eq!(2, len);
    assert_eq!(3, items.len());
    assert_eq!(123, items[0]);
    assert_eq!(456, items[1]);
    assert_eq!(789, items[2]);
}

#[test]
fn should_push_to_nested_array_256() {
    let root_address = SubAddress::new(0.into(), 0);
    let array_address = root_address.get_sub_address(0);
    let subarray_address = array_address.get_sub_address(0);

    let mut array: Array<Array<U256>> = Array::new(array_address);
    let mut sub_array = Array::new(subarray_address);

    sub_array.push(123.into());
    sub_array.push(456.into());
    array.push(sub_array);

    let mut sub_array = Array::new(array.address.get_sub_address(1));
    sub_array.push(789.into());
    array.push(sub_array);

    let len = array.len();
    let items: Vec<_> = array
        .into_iter()
        .map(|x| x.into_iter().collect::<Vec<_>>())
        .flatten()
        .collect();

    assert_eq!(2, len);
    assert_eq!(3, items.len());
    assert_eq!(123, items[0].as_u32());
    assert_eq!(456, items[1].as_u32());
    assert_eq!(789, items[2].as_u32());
}

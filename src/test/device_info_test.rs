extern crate alloc;

use alloc::vec;

use crate::DeviceInfo;
use crate::device_info::*;

#[test]
fn read_from_vec_device_info_reader_wrong_size_test() {
    let data = vec!();

    let err = DeviceInfoReader::read_from_vec(&data).unwrap_err();

    assert_eq!(DeviceInfoError::WrongSize, err);
}

#[test]
fn read_from_vec_device_info_reader_test() {
    let data = vec!(
        0x23, 0x01, 0x00, 0x00, // device_address
        0xab, 0x89, 0x67, 0x45, // firmware_version
        0x23, 0x01, 0xef, 0xcd, // event_processor_info_address
    );

    let device_info = DeviceInfoReader::read_from_vec(&data).unwrap();

    assert_eq!(device_info.device_address, 0x0123);
    assert_eq!(device_info.firmware_version, 0x456789ab);
    assert_eq!(device_info.event_processor_info_address, 0xcdef0123);
}

#[test]
fn write_to_vec_device_info_writer_test() {
    let device_info = DeviceInfo {
        device_address: 0x0123,
        firmware_version: 0x456789ab,
        event_processor_info_address: 0xcdef0123,
    };

    let mut data = vec!();

    DeviceInfoWriter::write_to_vec(&mut data, &device_info).unwrap();

    let expected_data = vec!(
        0x23, 0x01, 0x00, 0x00, // device_address
        0xab, 0x89, 0x67, 0x45, // firmware_version
        0x23, 0x01, 0xef, 0xcd, // event_processor_info_address
    );

    assert_eq!(data, expected_data);
}

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryInto;
use core::mem::{size_of, transmute, transmute_copy};

use crate::DeviceInfo;

const DEVICE_INFO_LEN: usize = size_of::<DeviceInfo>();

#[derive(Debug)]
pub enum DeviceInfoError {}

pub struct DeviceInfoReader {}

impl DeviceInfoReader {
    pub fn read_from_vec(data: &Vec<u8>) -> Result<DeviceInfo, DeviceInfoError> {
        let device_info: DeviceInfo = unsafe {
            transmute::<[u8; DEVICE_INFO_LEN], DeviceInfo>(data[..].try_into().unwrap())
        };

        Ok(device_info)
    }
}

pub struct DeviceInfoWriter {}

impl DeviceInfoWriter {
    pub fn write_to_vec(data: &mut Vec<u8>, device_info: &DeviceInfo) -> Result<(), DeviceInfoError> {
        unsafe {
            for byte in transmute_copy::<DeviceInfo, [u8; DEVICE_INFO_LEN]>(device_info).iter() {
                data.push(*byte);
            }
        };

        Ok(())
    }
}

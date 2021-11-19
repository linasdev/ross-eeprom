use core::convert::TryInto;
use core::mem::{size_of, transmute, transmute_copy};

use crate::DeviceInfo;

const DEVICE_INFO_LEN: usize = size_of::<DeviceInfo>();

#[derive(Debug)]
pub enum DeviceInfoError {}

pub struct DeviceInfoReader {}

impl DeviceInfoReader {
    pub fn read_from_array(data: &[u8; DEVICE_INFO_LEN]) -> Result<DeviceInfo, DeviceInfoError> {
        let device_info: DeviceInfo = unsafe {
            transmute::<[u8; DEVICE_INFO_LEN], DeviceInfo>(data[..].try_into().unwrap())
        };

        Ok(device_info)
    }
}

pub struct DeviceInfoWriter {}

impl DeviceInfoWriter {
    pub fn write_to_array(data: &mut [u8; DEVICE_INFO_LEN], device_info: &DeviceInfo) -> Result<(), DeviceInfoError> {
        *data = unsafe {
            transmute_copy(device_info)
        };

        Ok(())
    }
}

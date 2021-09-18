use core::mem::{size_of, transmute, transmute_copy};
use cortex_m::prelude::*;
use stm32f1xx_hal_bxcan::delay::Delay;
use eeprom24x::Eeprom24x;
use eeprom24x::page_size::B8;
use eeprom24x::addr_size::OneByte;

#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct DeviceInfo {
    pub device_address: u16,
    pub firmware_version: u32,
    pub peripheral_info_address: u32,
    pub program_info_address: u32,
}

#[derive(Debug)]
pub struct Eeprom<I2C, PS, AS> {
    driver: Eeprom24x<I2C, PS, AS,>,
    device_info_address: u32,
}

pub type EepromError = eeprom24x::Error<nb::Error<stm32f1xx_hal_bxcan::i2c::Error>>;

impl<I2C> Eeprom<I2C, B8, OneByte> where
    I2C: _embedded_hal_blocking_i2c_WriteRead<Error = nb::Error<stm32f1xx_hal_bxcan::i2c::Error>> + _embedded_hal_blocking_i2c_Write<Error = nb::Error<stm32f1xx_hal_bxcan::i2c::Error>>,
{
    pub fn new(driver: Eeprom24x<I2C, B8, OneByte>, device_info_address: u32) -> Self {
        Self {
            driver,
            device_info_address,
        }
    }

    pub fn read_device_info(&mut self) -> Result<DeviceInfo, EepromError> {
        let mut data = [0u8; size_of::<DeviceInfo>()];

        self.read_data(self.device_info_address, &mut data)?;

        let device_info = unsafe {
            transmute(data)
        };

        Ok(device_info)
    }

    pub fn write_device_info(&mut self, device_info: &DeviceInfo, delay: &mut Delay) -> Result<(), EepromError> {
        let data: [u8; size_of::<DeviceInfo>()] = unsafe {
            transmute_copy(device_info)
        };

        self.write_data(self.device_info_address, &data, delay)?;

        Ok(())
    }

    pub fn read_data(&mut self, address: u32, data: &mut [u8]) -> Result<(), EepromError> {
        loop {
            match self.driver.read_data(address, data) {
                Err(eeprom24x::Error::I2C(nb::Error::WouldBlock)) => continue,
                Err(err) => return Err(err),
                Ok(_) => return Ok(()),
            }
        }
    }

    pub fn write_data(&mut self, address: u32, data: &[u8], delay: &mut Delay) -> Result<(), EepromError> {
        let (page_count, slice_offset): (usize, usize) = if address % 8 == 0 {
            (
                (data.len() - 1) / 8 + 1,
                0
            )
        } else {
            (
                data.len() / 8,
                8 - (address as usize % 8),
            )
        };

        // Write part of the first page
        if slice_offset != 0 {
            let slice_end = if data.len() > slice_offset {
                slice_offset
            } else {
                data.len()
            };

            loop {
                match self.driver.write_page(address, &data[0..slice_end]) {
                    Err(eeprom24x::Error::I2C(nb::Error::WouldBlock)) => continue,
                    Err(err) => return Err(err),
                    Ok(_) => break,
                }
            }

            // Wait for eeprom
            delay.delay_ms(5u32);
        }

        // Write the rest in pages of 8 bytes
        for i in 0..page_count {
            let slice_start = i * 8 + slice_offset;
            let slice_end = if i == page_count - 1 {
                data.len()
            } else {
                (i + 1) * 8 + slice_offset
            };

            loop {
                match self.driver.write_page(address + (slice_start as u32), &data[slice_start..slice_end]) {
                    Err(eeprom24x::Error::I2C(nb::Error::WouldBlock)) => continue,
                    Err(err) => return Err(err),
                    Ok(_) => break,
                }
            }

            // Wait for eeprom
            delay.delay_ms(5u32);
        }

        Ok(())
    }
}

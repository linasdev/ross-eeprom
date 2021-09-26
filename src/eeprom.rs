extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::convert::TryInto;
use core::mem::{size_of, transmute, transmute_copy};
use cortex_m::prelude::*;
use stm32f1xx_hal_bxcan::delay::Delay;
use eeprom24x::Eeprom24x;
use eeprom24x::page_size::B32;
use eeprom24x::addr_size::TwoBytes;

use ross_config::event_processor::EventProcessor;
use ross_config::matcher::*;
use ross_config::extractor::*;
use ross_config::filter::*;
use ross_config::filter::state_filter::*;
use ross_config::producer::*;
use ross_config::producer::state_producer::*;

#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct DeviceInfo {
    pub device_address: u16,
    pub firmware_version: u32,
    pub event_processor_info_address: u32,
}

#[derive(Debug)]
pub struct Eeprom<I2C, PS, AS> {
    driver: Eeprom24x<I2C, PS, AS>,
    device_info_address: u32,
}

pub type EepromError = eeprom24x::Error<nb::Error<stm32f1xx_hal_bxcan::i2c::Error>>;

impl<I2C> Eeprom<I2C, B32, TwoBytes> where
    I2C: _embedded_hal_blocking_i2c_WriteRead<Error = nb::Error<stm32f1xx_hal_bxcan::i2c::Error>> + _embedded_hal_blocking_i2c_Write<Error = nb::Error<stm32f1xx_hal_bxcan::i2c::Error>>,
{
    pub fn new(driver: Eeprom24x<I2C, B32, TwoBytes>, device_info_address: u32) -> Self {
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

    pub fn read_event_processors(&mut self) -> Result<Vec<EventProcessor>, EepromError> {
        let device_info = self.read_device_info()?;

        let mut data = [0u8; size_of::<u32>()];
        self.read_data(device_info.event_processor_info_address, &mut data)?;

        let data_len = u32::from_be_bytes(data[0..=3].try_into().unwrap());
        let mut data = vec!();
        data.resize(data_len as usize, 0x00); 

        self.read_data(device_info.event_processor_info_address + size_of::<u32>() as u32, &mut data)?;

        let event_processor_count = u32::from_be_bytes(data[0..=3].try_into().unwrap());
        let mut event_processors = vec!();
        event_processors.reserve(event_processor_count as usize);

        let mut offset = size_of::<u32>();

        for _ in 0..event_processor_count {
            let matcher_count = u32::from_be_bytes(data[offset..offset + size_of::<u32>()].try_into().unwrap());
            offset += size_of::<u32>();

            let mut matchers = vec!();
            matchers.reserve(matcher_count as usize);

            for _ in 0..matcher_count {
                let extractor_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
                offset += size_of::<u16>();

                let extractor = Self::read_extractor_from_vec(&data, &mut offset, extractor_code);

                let filter_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
                offset += size_of::<u16>();

                let filter = Self::read_filter_from_vec(&data, &mut offset, filter_code);

                matchers.push(Matcher {
                    extractor,
                    filter,
                });
            }

            let extractor_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
            offset += size_of::<u16>();

            let extractor = Self::read_extractor_from_vec(&data, &mut offset, extractor_code);

            let producer_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
            offset += size_of::<u16>();

            let producer = Self::read_producer_from_vec(&data, &mut offset, producer_code);

            event_processors.push(EventProcessor {
                matchers,
                extractor,
                producer,
            });
        }

        Ok(event_processors)
    }

    pub fn write_event_processors(&mut self, event_processors: &Vec<EventProcessor>, delay: &mut Delay) -> Result<(), EepromError> {
        let mut data = vec!();

        for byte in u32::to_be_bytes(event_processors.len() as u32).iter() {
            data.push(*byte);
        }

        for event_processor in event_processors.iter() {
            for byte in u32::to_be_bytes(event_processor.matchers.len() as u32).iter() {
                data.push(*byte);
            }

            for matcher in event_processor.matchers.iter() {
                Self::write_extractor_to_vec(&mut data, &matcher.extractor);
                Self::write_filter_to_vec(&mut data, &matcher.filter);
            }
            
            Self::write_extractor_to_vec(&mut data, &event_processor.extractor);
            Self::write_producer_to_vec(&mut data, &event_processor.producer);
        }

        for (i, byte) in u32::to_be_bytes(data.len() as u32).iter().enumerate() {
            data.insert(i, *byte);
        }

        let device_info = self.read_device_info()?;
        self.write_data(device_info.event_processor_info_address, &data, delay)?;

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

    fn read_extractor_from_vec(data: &Vec<u8>, offset: &mut usize, extractor_code: u16) -> Box<dyn Extractor> {
        match extractor_code {
            NONE_EXTRACTOR_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<NoneExtractor>();
                    let extractor = Box::new(transmute_copy::<[u8; SIZE], NoneExtractor>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return extractor;
                }
            },
            EVENT_CODE_EXTRACTOR_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<EventCodeExtractor>();
                    let extractor = Box::new(transmute_copy::<[u8; SIZE], EventCodeExtractor>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return extractor;
                }
            },
            _ => panic!("Unknown extractor."),
        }
    }

    fn write_extractor_to_vec(data: &mut Vec<u8>, extractor: &Box<dyn Extractor>) {
        if let Some(extractor) = extractor.downcast_ref::<NoneExtractor>() {
            Self::write_u16_to_vec(data, NONE_EXTRACTOR_CODE);

            unsafe {
                for byte in transmute_copy::<NoneExtractor, [u8; size_of::<NoneExtractor>()]>(extractor).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(extractor) = extractor.downcast_ref::<EventCodeExtractor>() {
            Self::write_u16_to_vec(data, EVENT_CODE_EXTRACTOR_CODE);

            unsafe {
                for byte in transmute_copy::<EventCodeExtractor, [u8; size_of::<EventCodeExtractor>()]>(extractor).iter() {
                    data.push(*byte);
                }
            }
        } else {
            panic!("Unknown extractor.");
        }
    }

    fn read_filter_from_vec(data: &Vec<u8>, offset: &mut usize, filter_code: u16) -> Box<dyn Filter> {
        match filter_code {
            U8_INCREMENT_STATE_FILTER => {
                unsafe {
                    const SIZE: usize = size_of::<U8IncrementStateFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], U8IncrementStateFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            U16_IS_EQUAL_FILTER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<U16IsEqualFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], U16IsEqualFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            U32_IS_EQUAL_STATE_FILTER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<U32IsEqualStateFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], U32IsEqualStateFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            U32_INCREMENT_STATE_FILTER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<U32IncrementStateFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], U32IncrementStateFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            U32_SET_STATE_FILTER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<U32SetStateFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], U32SetStateFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            FLIP_FLOP_FILTER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<FlipFlopFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], FlipFlopFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            COUNT_FILTER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<CountFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], CountFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            COUNT_STATE_FILTER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<CountStateFilter>();
                    let filter = Box::new(transmute_copy::<[u8; SIZE], CountStateFilter>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return filter;
                }
            },
            _ => panic!("Unknown filter."),
        }
    }

    fn write_filter_to_vec(data: &mut Vec<u8>, filter: &Box<dyn Filter>) {
        if let Some(filter) = filter.downcast_ref::<U8IncrementStateFilter>() {
            Self::write_u16_to_vec(data, U8_INCREMENT_STATE_FILTER);

            unsafe {
                for byte in transmute_copy::<U8IncrementStateFilter, [u8; size_of::<U8IncrementStateFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(filter) = filter.downcast_ref::<U16IsEqualFilter>() {
            Self::write_u16_to_vec(data, U16_IS_EQUAL_FILTER_CODE);

            unsafe {
                for byte in transmute_copy::<U16IsEqualFilter, [u8; size_of::<U16IsEqualFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(filter) = filter.downcast_ref::<U32IsEqualStateFilter>() {
            Self::write_u16_to_vec(data, U32_IS_EQUAL_STATE_FILTER_CODE);

            unsafe {
                for byte in transmute_copy::<U32IsEqualStateFilter, [u8; size_of::<U32IsEqualStateFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(filter) = filter.downcast_ref::<U32IncrementStateFilter>() {
            Self::write_u16_to_vec(data, U32_INCREMENT_STATE_FILTER_CODE);

            unsafe {
                for byte in transmute_copy::<U32IncrementStateFilter, [u8; size_of::<U32IncrementStateFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(filter) = filter.downcast_ref::<U32SetStateFilter>() {
            Self::write_u16_to_vec(data, U32_SET_STATE_FILTER_CODE);

            unsafe {
                for byte in transmute_copy::<U32SetStateFilter, [u8; size_of::<U32SetStateFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(filter) = filter.downcast_ref::<FlipFlopFilter>() {
            Self::write_u16_to_vec(data, FLIP_FLOP_FILTER_CODE);

            unsafe {
                for byte in transmute_copy::<FlipFlopFilter, [u8; size_of::<FlipFlopFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(filter) = filter.downcast_ref::<CountFilter>() {
            Self::write_u16_to_vec(data, COUNT_FILTER_CODE);

            unsafe {
                for byte in transmute_copy::<CountFilter, [u8; size_of::<CountFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(filter) = filter.downcast_ref::<CountStateFilter>() {
            Self::write_u16_to_vec(data, COUNT_STATE_FILTER_CODE);

            unsafe {
                for byte in transmute_copy::<CountStateFilter, [u8; size_of::<CountStateFilter>()]>(filter).iter() {
                    data.push(*byte);
                }
            }
        } else {
            panic!("Unknown filter.");
        }
    }

    fn read_producer_from_vec(data: &Vec<u8>, offset: &mut usize, extractor_code: u16) -> Box<dyn Producer> {
        match extractor_code {
            BCM_CHANGE_BRIGHTNESS_PRODUCER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<BcmChangeBrightnessProducer>();
                    let producer = Box::new(transmute_copy::<[u8; SIZE], BcmChangeBrightnessProducer>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return producer;
                }
            },
            BCM_CHANGE_BRIGHTNESS_STATE_PRODUCER_CODE => {
                unsafe {
                    const SIZE: usize = size_of::<BcmChangeBrightnessStateProducer>();
                    let producer = Box::new(transmute_copy::<[u8; SIZE], BcmChangeBrightnessStateProducer>(data[*offset..*offset + SIZE].try_into().unwrap()));
                    *offset += SIZE;

                    return producer;
                }
            },
            _ => panic!("Unknown producer."),
        }
    }

    fn write_producer_to_vec(data: &mut Vec<u8>, producer: &Box<dyn Producer>) {
        if let Some(producer) = producer.downcast_ref::<NoneProducer>() {
            Self::write_u16_to_vec(data, NONE_PRODUCER_CODE);

            unsafe {
                for byte in transmute_copy::<NoneProducer, [u8; size_of::<NoneProducer>()]>(producer).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(producer) = producer.downcast_ref::<BcmChangeBrightnessProducer>() {
            Self::write_u16_to_vec(data, BCM_CHANGE_BRIGHTNESS_PRODUCER_CODE);

            unsafe {
                for byte in transmute_copy::<BcmChangeBrightnessProducer, [u8; size_of::<BcmChangeBrightnessProducer>()]>(producer).iter() {
                    data.push(*byte);
                }
            }
        } else if let Some(producer) = producer.downcast_ref::<BcmChangeBrightnessStateProducer>() {
            Self::write_u16_to_vec(data, BCM_CHANGE_BRIGHTNESS_STATE_PRODUCER_CODE);

            unsafe {
                for byte in transmute_copy::<BcmChangeBrightnessStateProducer, [u8; size_of::<BcmChangeBrightnessStateProducer>()]>(producer).iter() {
                    data.push(*byte);
                }
            }
        } else {
            panic!("Unknown producer.");
        }
    }

    fn write_u16_to_vec(data: &mut Vec<u8>, value: u16) {
        for byte in u16::to_be_bytes(value).iter() {
            data.push(*byte);
        }
    }
}

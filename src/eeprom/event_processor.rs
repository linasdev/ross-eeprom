extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::convert::TryInto;
use core::mem::{size_of, transmute_copy};

use ross_config::event_processor::EventProcessor;
use ross_config::matcher::Matcher;
use ross_config::extractor::*;
use ross_config::filter::*;
use ross_config::filter::state_filter::*;
use ross_config::producer::*;
use ross_config::producer::state_producer::*;

macro_rules! impl_item_read {
    ($item_code:expr, $item_type:ty, $data:expr, $offset:expr, $provided_code:expr) => {
        if $item_code == $provided_code {
            unsafe {
                const SIZE: usize = size_of::<$item_type>();
                let item = Box::new(transmute_copy::<[u8; SIZE], $item_type>($data[*$offset..*$offset + SIZE].try_into().unwrap()));
                *$offset += SIZE;

                return Ok(item);
            }
        }
    };
}

macro_rules! impl_item_write {
    ($item_code:expr, $item_type:ty, $data:expr, $item:expr) => {
        if let Some(item) = $item.downcast_ref::<$item_type>() {
            Self::write_u16_to_vec($data, $item_code);
    
            unsafe {
                for byte in transmute_copy::<$item_type, [u8; size_of::<$item_type>()]>(item).iter() {
                    $data.push(*byte);
                }
            }

            return Ok(());
        }
    };
}

#[derive(Debug, PartialEq)]
pub enum EventProcessorError {
    WrongSize,
    UnknownExtractor,
    UnknownFilter,
    UnknownProducer,
}

pub struct EventProcessorReader {}

impl EventProcessorReader {
    pub fn read_from_vec(data: &Vec<u8>) -> Result<Vec<EventProcessor>, EventProcessorError> {
        if data.len() < 4 {
            return Err(EventProcessorError::WrongSize);
        }

        let event_processor_count = u32::from_be_bytes(data[0..=3].try_into().unwrap());
        let mut event_processors = vec!();
        event_processors.reserve(event_processor_count as usize);
    
        let mut offset = size_of::<u32>();
    
        for _ in 0..event_processor_count {
            if data.len() < offset + size_of::<u32>() {
                return Err(EventProcessorError::WrongSize);
            }

            let matcher_count = u32::from_be_bytes(data[offset..offset + size_of::<u32>()].try_into().unwrap());
            offset += size_of::<u32>();
    
            let mut matchers = vec!();
            matchers.reserve(matcher_count as usize);
    
            for _ in 0..matcher_count {
                if data.len() < offset + size_of::<u16>() {
                    return Err(EventProcessorError::WrongSize);
                }

                let extractor_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
                offset += size_of::<u16>();
    
                let extractor = Self::read_extractor_from_vec(data, &mut offset, extractor_code)?;
    
                if data.len() < offset + size_of::<u16>() {
                    return Err(EventProcessorError::WrongSize);
                }

                let filter_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
                offset += size_of::<u16>();
    
                let filter = Self::read_filter_from_vec(data, &mut offset, filter_code)?;
    
                matchers.push(Matcher {
                    extractor,
                    filter,
                });
            }
    
            if data.len() < offset + size_of::<u16>() {
                return Err(EventProcessorError::WrongSize);
            }

            let extractor_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
            offset += size_of::<u16>();
    
            let extractor = Self::read_extractor_from_vec(data, &mut offset, extractor_code)?;
    
            if data.len() < offset + size_of::<u16>() {
                return Err(EventProcessorError::WrongSize);
            }

            let producer_code = u16::from_be_bytes(data[offset..offset + size_of::<u16>()].try_into().unwrap());
            offset += size_of::<u16>();
    
            let producer = Self::read_producer_from_vec(data, &mut offset, producer_code)?;
    
            event_processors.push(EventProcessor {
                matchers,
                extractor,
                producer,
            });
        }
    
        Ok(event_processors)
    }

    fn read_extractor_from_vec(data: &Vec<u8>, offset: &mut usize, extractor_code: u16) -> Result<Box<dyn Extractor>, EventProcessorError> {
        impl_item_read!(NONE_EXTRACTOR_CODE, NoneExtractor, data, offset, extractor_code);
        impl_item_read!(EVENT_CODE_EXTRACTOR_CODE, EventCodeExtractor, data, offset, extractor_code);
        Err(EventProcessorError::UnknownExtractor)
    }

    fn read_filter_from_vec(data: &Vec<u8>, offset: &mut usize, filter_code: u16) -> Result<Box<dyn Filter>, EventProcessorError> {
        impl_item_read!(U8_INCREMENT_STATE_FILTER, U8IncrementStateFilter, data, offset, filter_code);
        impl_item_read!(U16_IS_EQUAL_FILTER_CODE, U16IsEqualFilter, data, offset, filter_code);
        impl_item_read!(U32_IS_EQUAL_STATE_FILTER_CODE, U32IsEqualStateFilter, data, offset, filter_code);
        impl_item_read!(U32_INCREMENT_STATE_FILTER_CODE, U32IncrementStateFilter, data, offset, filter_code);
        impl_item_read!(U32_SET_STATE_FILTER_CODE, U32SetStateFilter, data, offset, filter_code);
        impl_item_read!(FLIP_FLOP_FILTER_CODE, FlipFlopFilter, data, offset, filter_code);
        impl_item_read!(COUNT_FILTER_CODE, CountFilter, data, offset, filter_code);
        impl_item_read!(COUNT_STATE_FILTER_CODE, CountStateFilter, data, offset, filter_code);
        Err(EventProcessorError::UnknownFilter)
    }

    fn read_producer_from_vec(data: &Vec<u8>, offset: &mut usize, producer_code: u16) -> Result<Box<dyn Producer>, EventProcessorError> {
        impl_item_read!(NONE_PRODUCER_CODE, NoneProducer, data, offset, producer_code);
        impl_item_read!(BCM_CHANGE_BRIGHTNESS_PRODUCER_CODE, BcmChangeBrightnessProducer, data, offset, producer_code);
        impl_item_read!(BCM_CHANGE_BRIGHTNESS_STATE_PRODUCER_CODE, BcmChangeBrightnessStateProducer, data, offset, producer_code);
        Err(EventProcessorError::UnknownProducer)
    }
}

pub struct EventProcessorWriter {}

impl EventProcessorWriter {
    pub fn write_to_vec(data: &mut Vec<u8>, event_processors: &Vec<EventProcessor>) -> Result<(), EventProcessorError> {
        for byte in u32::to_be_bytes(event_processors.len() as u32).iter() {
            data.push(*byte);
        }
    
        for event_processor in event_processors.iter() {
            for byte in u32::to_be_bytes(event_processor.matchers.len() as u32).iter() {
                data.push(*byte);
            }
    
            for matcher in event_processor.matchers.iter() {
                Self::write_extractor_to_vec(data, &matcher.extractor)?;
                Self::write_filter_to_vec(data, &matcher.filter)?;
            }
            
            Self::write_extractor_to_vec(data, &event_processor.extractor)?;
            Self::write_producer_to_vec(data, &event_processor.producer)?;
        }
    
        Ok(())
    }

    pub fn write_extractor_to_vec(data: &mut Vec<u8>, extractor: &Box<dyn Extractor>) -> Result<(), EventProcessorError> {
        impl_item_write!(NONE_EXTRACTOR_CODE, NoneExtractor, data, extractor);
        impl_item_write!(EVENT_CODE_EXTRACTOR_CODE, EventCodeExtractor, data, extractor);
        Err(EventProcessorError::UnknownExtractor)
    }

    fn write_filter_to_vec(data: &mut Vec<u8>, filter: &Box<dyn Filter>) -> Result<(), EventProcessorError> {
        impl_item_write!(U8_INCREMENT_STATE_FILTER, U8IncrementStateFilter, data, filter);
        impl_item_write!(U16_IS_EQUAL_FILTER_CODE, U16IsEqualFilter, data, filter);
        impl_item_write!(U32_IS_EQUAL_STATE_FILTER_CODE, U32IsEqualStateFilter, data, filter);
        impl_item_write!(U32_INCREMENT_STATE_FILTER_CODE, U32IncrementStateFilter, data, filter);
        impl_item_write!(U32_SET_STATE_FILTER_CODE, U32SetStateFilter, data, filter);
        impl_item_write!(FLIP_FLOP_FILTER_CODE, FlipFlopFilter, data, filter);
        impl_item_write!(COUNT_FILTER_CODE, CountFilter, data, filter);
        impl_item_write!(COUNT_STATE_FILTER_CODE, CountStateFilter, data, filter);
        Err(EventProcessorError::UnknownFilter)
    }

    fn write_producer_to_vec(data: &mut Vec<u8>, producer: &Box<dyn Producer>) -> Result<(), EventProcessorError> {
        impl_item_write!(NONE_PRODUCER_CODE, NoneProducer, data, producer);
        impl_item_write!(BCM_CHANGE_BRIGHTNESS_PRODUCER_CODE, BcmChangeBrightnessProducer, data, producer);
        impl_item_write!(BCM_CHANGE_BRIGHTNESS_STATE_PRODUCER_CODE, BcmChangeBrightnessStateProducer, data, producer);
        Err(EventProcessorError::UnknownProducer)
    }

    fn write_u16_to_vec(data: &mut Vec<u8>, value: u16) {
        for byte in u16::to_be_bytes(value).iter() {
            data.push(*byte);
        }
    }
}

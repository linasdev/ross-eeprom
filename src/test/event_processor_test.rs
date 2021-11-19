extern crate alloc;

use alloc::vec;
use alloc::boxed::Box;

use ross_config::event_processor::EventProcessor;
use ross_config::matcher::Matcher;
use ross_config::extractor::NoneExtractor;
use ross_config::filter::FlipFlopFilter;
use ross_config::producer::NoneProducer;

use crate::event_processor::*;

#[test]
fn read_from_vec_event_processor_reader_wrong_size_test() {
    let data = vec!();

    let err = EventProcessorReader::read_from_vec(&data).unwrap_err();

    assert_eq!(EventProcessorError::WrongSize, err);
}

#[test]
fn read_from_vec_event_processor_reader_empty_test() {
    let data = vec!(
        0x00, 0x00, 0x00, 0x00  // event processor count
    );

    let event_processors = EventProcessorReader::read_from_vec(&data).unwrap();

    assert_eq!(event_processors.len(), 0);
}

#[test]
fn read_from_vec_event_processor_reader_test() {
    let data = vec!(
        0x00, 0x00, 0x00, 0x01, // event processor count
        0x00, 0x00, 0x00, 0x01, // matcher count
        0x00, 0x00,             // none extractor code
        0x00, 0x05,             // flip flop filter code
        0x00,                   // flip flop filter state
        0x00, 0x00,             // none extractor code
        0x00, 0x00,             // none producer code
    );

    let event_processors = EventProcessorReader::read_from_vec(&data).unwrap();

    assert_eq!(event_processors.len(), 1);
}

#[test]
fn write_to_vec_event_processor_writer_test() {
    let event_processors = vec!(EventProcessor {
        matchers: vec!(Matcher {
            extractor: Box::new(NoneExtractor::new()),
            filter: Box::new(FlipFlopFilter::new(false)),
        }),
        extractor: Box::new(NoneExtractor::new()),
        producer: Box::new(NoneProducer::new()),
    });

    let mut data = vec!();

    EventProcessorWriter::write_to_vec(&mut data, &event_processors).unwrap();

    let expected_data = vec!(
        0x00, 0x00, 0x00, 0x01, // event processor count
        0x00, 0x00, 0x00, 0x01, // matcher count
        0x00, 0x00,             // none extractor code
        0x00, 0x05,             // flip flop filter code
        0x00,                   // flip flop filter state
        0x00, 0x00,             // none extractor code
        0x00, 0x00,             // none producer code
    );

    assert_eq!(data, expected_data);
}

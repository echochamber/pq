#![deny(missing_docs)]

use crate::error::*;
use crate::stream::FramedRead;
use kafka::consumer::{Consumer, FetchOffset};
use std;
use std::collections::VecDeque;

/// A consumer from Kafka
pub struct KafkaConsumer {
    consumer: Consumer,
    messages: VecDeque<Vec<u8>>,
}

impl FramedRead for KafkaConsumer {
    fn read_next_frame<'a>(
        &mut self,
        buffer: &'a mut Vec<u8>,
    ) -> std::io::Result<Option<&'a [u8]>> {
        let res = self.next().map(move |mut v| {
            std::mem::swap(&mut v, buffer);
            &buffer[..]
        });
        Ok(res)
    }
}

impl Iterator for KafkaConsumer {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        if self.messages.is_empty() {
            let kafka_consumer = &mut self.consumer;
            loop {
                match kafka_consumer.poll() {
                    Ok(mss) => {
                        for ms in mss.iter() {
                            self.messages.append(
                                &mut ms
                                    .messages()
                                    .iter()
                                    .map(|z| z.value.to_vec())
                                    .collect::<VecDeque<_>>(),
                            );
                            kafka_consumer
                                .consume_messageset(ms)
                                .expect("Couldn't mark messageset as consumed");
                        }
                        kafka_consumer
                            .commit_consumed()
                            .expect("Couldn't commit consumption");
                        if !self.messages.is_empty() {
                            break;
                        }
                    }
                    Err(_) => return None,
                }
            }
        }
        self.messages.pop_front()
    }
}

impl KafkaConsumer {
    /// Return a KafkaConsumer with some basic kafka connection properties
    pub fn new(brokers: &str, topic: &str, from_beginning: bool) -> Result<KafkaConsumer> {
        let fetch_offset = if from_beginning {
            FetchOffset::Earliest
        } else {
            FetchOffset::Latest
        };
        match Consumer::from_hosts(
            brokers
                .split(',')
                .map(std::borrow::ToOwned::to_owned)
                .collect::<Vec<String>>(),
        )
        .with_topic(topic.to_owned())
        .with_fallback_offset(fetch_offset)
        .create()
        {
            Ok(consumer) => Ok(KafkaConsumer {
                consumer,
                messages: VecDeque::new(),
            }),
            Err(e) => Err(StreamDelimitError::KafkaInitializeError(e)),
        }
    }
}

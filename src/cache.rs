use crate::kv;
use domain::base::question::Question;
use domain::base::rdata::UnknownRecordData;
use domain::base::{Dname, Record};
use js_sys::Date;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct DnsCacheMetadata {
    created_ts: u64, // seconds
    ttl: u32,
}

pub struct DnsCache {
    store: kv::KvNamespace,
}

impl DnsCache {
    pub fn new() -> DnsCache {
        DnsCache {
            store: kv::get_dns_cache(),
        }
    }

    pub async fn put_cache(
        &self,
        record: &Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>,
    ) -> Result<(), String> {
        let ttl = record.ttl();
        self.store
            .put_buf_ttl_metadata(
                &Self::record_to_key(record),
                record.data().data(),
                ttl as u64,
                DnsCacheMetadata {
                    created_ts: (Date::now() / 1000f64) as u64,
                    ttl,
                },
            )
            .await
    }

    pub async fn get_cache(
        &self,
        question: &Question<Dname<Vec<u8>>>,
    ) -> Option<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>> {
        let (value, metadata): (Option<Vec<u8>>, Option<DnsCacheMetadata>) = self
            .store
            .get_buf_metadata(&Self::question_to_key(question))
            .await;
        if value.is_none() || metadata.is_none() {
            return None;
        }

        let (value, metadata) = (value.unwrap(), metadata.unwrap());
        let elapsed_since_creation = (Date::now() / 1000f64) as u64 - metadata.created_ts;
        // Calculate the remaining TTL correctly
        // don't just return the original TTL blindly
        let remaining_ttl = if elapsed_since_creation > metadata.ttl as u64 {
            0
        } else {
            metadata.ttl as u64 - elapsed_since_creation
        };

        Some(Record::new(
            question.qname().to_owned(),
            question.qclass(),
            remaining_ttl as u32,
            UnknownRecordData::from_octets(question.qtype(), value),
        ))
    }

    fn record_to_key(record: &Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>) -> String {
        format!("{};{};{}", record.owner(), record.rtype(), record.class())
    }

    fn question_to_key(question: &Question<Dname<Vec<u8>>>) -> String {
        format!(
            "{};{};{}",
            question.qname(),
            question.qtype(),
            question.qclass()
        )
    }
}

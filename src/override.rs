use domain::base::rdata::UnknownRecordData;
use domain::base::{question::Question, Compose};
use domain::base::{Dname, Record, Rtype};
use domain::rdata::{Aaaa, AllRecordData, A};
use std::collections::HashMap;
use std::net::IpAddr;

pub struct OverrideResolver {
    simple_matches: HashMap<String, IpAddr>,
    override_ttl: u32,
}

impl OverrideResolver {
    pub fn new(overrides: HashMap<String, String>, override_ttl: u32) -> OverrideResolver {
        OverrideResolver {
            simple_matches: Self::build_simple_match_table(overrides),
            override_ttl,
        }
    }

    fn build_simple_match_table(overrides: HashMap<String, String>) -> HashMap<String, IpAddr> {
        let mut ret = HashMap::new();
        for (k, v) in overrides.into_iter() {
            match v.parse::<IpAddr>() {
                Ok(addr) => {
                    ret.insert(k, addr);
                }
                // Ignore malformed IP addresses
                Err(_) => continue,
            }
        }
        return ret;
    }

    pub fn try_resolve(
        &self,
        question: &Question<Dname<Vec<u8>>>,
    ) -> Option<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>> {
        match question.qtype() {
            // We only handle resolution of IP addresses
            Rtype::A | Rtype::A6 | Rtype::Aaaa | Rtype::Cname | Rtype::Any => (),
            // So if the question is anything else, just skip
            _ => return None,
        }

        let name = question.qname().to_string();
        if let Some(addr) = self.simple_matches.get(&name) {
            self.respond_with_addr(question, addr)
        } else {
            None
        }
    }

    fn respond_with_addr(
        &self,
        question: &Question<Dname<Vec<u8>>>,
        addr: &IpAddr,
    ) -> Option<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>> {
        let (rtype, rdata): (_, AllRecordData<Vec<u8>, Dname<Vec<u8>>>) = match addr {
            IpAddr::V4(addr) => (Rtype::A, AllRecordData::A(A::new(addr.clone()))),
            IpAddr::V6(addr) => (Rtype::Aaaa, AllRecordData::Aaaa(Aaaa::new(addr.clone()))),
        };

        let qtype = question.qtype();
        if qtype == Rtype::Any || qtype == rtype {
            // Convert AllRecordData to UnknownRecordData to match the type
            // since our resolver client doesn't really care about the actual type
            let mut rdata_buf: Vec<u8> = Vec::new();
            rdata.compose(&mut rdata_buf).ok()?;
            let record = Record::new(
                question.qname().clone(),
                question.qclass(),
                self.override_ttl,
                UnknownRecordData::from_octets(rtype, rdata_buf),
            );
            return Some(record);
        } else {
            // If the response and query types don't match, just return none
            return None;
        }
    }
}

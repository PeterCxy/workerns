use crate::trie_map::TrieMap;
use domain::base::{rdata::UnknownRecordData, Compose, Dname, Question, Record, Rtype};
use domain::rdata::{Aaaa, AllRecordData, A};
use std::collections::HashMap;
use std::net::IpAddr;

pub struct OverrideResolver {
    simple_matches: HashMap<String, IpAddr>,
    suffix_matches: TrieMap<IpAddr>,
    override_ttl: u32,
}

impl OverrideResolver {
    pub fn new(overrides: HashMap<String, String>, override_ttl: u32) -> OverrideResolver {
        let (simple_matches, suffix_matches) = Self::build_match_tables(overrides);
        OverrideResolver {
            suffix_matches,
            simple_matches,
            override_ttl,
        }
    }

    fn build_match_tables(
        overrides: HashMap<String, String>,
    ) -> (HashMap<String, IpAddr>, TrieMap<IpAddr>) {
        let mut simple = HashMap::new();
        let mut suffix = TrieMap::new();
        for (k, v) in overrides.into_iter() {
            match v.parse::<IpAddr>() {
                Ok(addr) => {
                    if k.starts_with("*.") {
                        // Anything starting with a wildcard character is a suffix match
                        // we convert it to a prefix match by reversing the domain
                        // Note that we get rid of the wildcard but keep the dot, i.e.
                        // we don't allow suffix match in the middle of a part of a domain
                        suffix.put_prefix(k[1..].chars().rev().collect::<String>(), addr);
                    } else {
                        simple.insert(k, addr);
                    }
                }
                // Ignore malformed IP addresses
                Err(_) => continue,
            }
        }
        (simple, suffix)
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
        } else if let Some(addr) = self
            .suffix_matches
            .get_prefix(name.chars().rev().collect::<String>())
        {
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

        // Convert AllRecordData to UnknownRecordData to match the type signature
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
    }
}

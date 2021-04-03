use domain_core::bits::ParsedDname;
use domain_core::bits::message::Message;
use domain_core::bits::question::Question;
use domain_core::bits::record::ParsedRecord;

pub struct ClientOptions {
    pub upstream_urls: Vec<String>
}

// The DNS client implementation
pub struct Client {
    options: ClientOptions
}

impl Client {
    pub fn new(options: ClientOptions) -> Client {
        Client { options }
    }

    pub async fn query(&self, questions: Vec<Question<ParsedDname>>) -> Result<Vec<ParsedRecord>, String> {
        unimplemented!()
    }
}
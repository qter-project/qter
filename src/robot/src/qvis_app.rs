use puzzle_theory::permutations::Permutation;
use reqwest::{blocking::Client, retry};
use std::time::Duration;

const TAKE_PICTURE_ENDPOINT: &str = "/api/take_picture";
const MAX_RETRIES: u32 = 5;
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct QvisAppHandle {
    endpoint: String,
    client: Client,
}

impl QvisAppHandle {
    pub fn init(port: u16) -> Self {
        let endpoint = format!("127.0.0.1:{}{}", port, TAKE_PICTURE_ENDPOINT);
        let client = Client::builder()
            .retry(retry::for_host(endpoint.clone()).max_retries_per_request(MAX_RETRIES))
            .timeout(TIMEOUT)
            .build()
            .unwrap();
        QvisAppHandle { endpoint, client }
    }

    pub fn take_picture(&self) -> Permutation {
        self.client
            .get(&self.endpoint)
            .send()
            .unwrap()
            .json::<Permutation>()
            .unwrap()
    }
}

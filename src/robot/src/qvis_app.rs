use puzzle_theory::permutations::Permutation;
use reqwest::{Url, blocking::Client, retry};

const TAKE_PICTURE_ENDPOINT: &str = "/api/take_picture";
const MAX_RETRIES: u32 = 5;

pub struct QvisAppHandle {
    endpoint: String,
    client: Client,
}

impl QvisAppHandle {
    pub fn init() -> Self {
        let endpoint = format!("http://127.0.0.1{}", TAKE_PICTURE_ENDPOINT);
        let client = Client::builder()
            .retry(retry::for_host(endpoint.clone()).max_retries_per_request(MAX_RETRIES))
            .timeout(None)
            .build()
            .unwrap();
        QvisAppHandle { endpoint, client }
    }

    pub fn take_picture(&self, maybe_calibration_permutation: Option<Permutation>) -> Permutation {
        let endpoint = if let Some(calibration_permutation) = maybe_calibration_permutation {
            Url::parse_with_params(
                &self.endpoint,
                &[(
                    "calibration_permutation",
                    calibration_permutation.to_string(),
                )],
            )
            .unwrap()
        } else {
            Url::parse(&self.endpoint).unwrap()
        };
        self.client
            .get(endpoint)
            .send()
            .unwrap()
            .json::<Permutation>()
            .unwrap()
    }
}

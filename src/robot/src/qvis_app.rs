use puzzle_theory::permutations::Permutation;
use reqwest::{Client, Url, retry};

const TAKE_PICTURE_ENDPOINT: &str = "/api/take_picture";
const MAX_RETRIES: u32 = 5;

pub struct QvisAppHandle {
    endpoint: String,
    client: Client,
}

impl QvisAppHandle {
    pub fn init() -> Self {
        let endpoint = format!("http://127.0.0.1:3000{}", TAKE_PICTURE_ENDPOINT);
        let client = Client::builder()
            .retry(retry::for_host(endpoint.clone()).max_retries_per_request(MAX_RETRIES))
            .build()
            .unwrap();
        QvisAppHandle { endpoint, client }
    }

    pub async fn calibrate_permutation(
        &self,
        calibration_permutation: Permutation,
    ) -> Result<(), reqwest::Error> {
        self.client
            .get(
                Url::parse_with_params(
                    &self.endpoint,
                    &[(
                        "calibration_permutation",
                        calibration_permutation.to_string(),
                    )],
                )
                .unwrap(),
            )
            .send()
            .await?
            .json::<Option<Permutation>>()
            .await
            .map(|p| {
                assert!(p.is_none());
            })
    }

    pub async fn take_picture(&self) -> Result<Permutation, reqwest::Error> {
        self.client
            .get(&self.endpoint)
            .send()
            .await?
            .json::<Option<Permutation>>()
            .await
            .map(Option::unwrap)
    }
}

use puzzle_theory::permutations::Permutation;

const PORT: u16 = 3000;

pub struct QvisAppHandle {
    endpoint: String,
}

impl QvisAppHandle {
    pub fn init() -> Self {
        todo!();
        // QvisAppHandle { endpoint, client }
    }

    pub async fn calibrate_permutation(
        &self,
        calibration_permutation: Permutation,
    ) -> Result<(), String> {
        todo!();
        // self.client
        //     .get(
        //         Url::parse_with_params(
        //             &self.endpoint,
        //             &[(
        //                 "calibration_permutation",
        //                 calibration_permutation.to_string(),
        //             )],
        //         )
        //         .unwrap(),
        //     )
        //     .send()
        //     .await?
        //     .json::<Option<Permutation>>()
        //     .await
        //     .map(|p| {
        //         assert!(p.is_none());
        //     })
        
    }

    pub async fn take_picture(&self) -> Result<Permutation, String> {
        todo!();
        // self.client
        //     .get(&self.endpoint)
        //     .send()
        //     .await?
        //     .json::<Option<Permutation>>()
        //     .await
        //     .map(Option::unwrap)
    }
}

use yarapi::rest::activity::DefaultActivity;

pub struct ProverRunner {
    activity: DefaultActivity,
    prover_id: i32,
}

impl ProverRunner {
    pub fn new(activity: DefaultActivity, id: i32) -> ProverRunner {
        ProverRunner {
            activity,
            prover_id: id,
        }
    }
}

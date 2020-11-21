use zksync_prover::cli_utils::{main_prover_internal, Opt};
use zksync_prover::plonk_step_by_step_prover::PlonkStepByStepProver;
use zksync_utils::parse_env;

mod client;
use crate::client::YagnaApiClient;

use std::time::Duration;
use structopt::StructOpt;

fn main() {
    let opt = Opt::from_args();
    let worker_name = opt.worker_name;

    // Doesn't matter. We don't communicate with any server.
    let server_api_url = parse_env("PROVER_SERVER_URL");
    let request_timout = Duration::from_secs(parse_env::<u64>("REQ_SERVER_TIMEOUT"));
    let api_client = YagnaApiClient::new(&server_api_url, &worker_name, request_timout);

    main_prover_internal::<YagnaApiClient, PlonkStepByStepProver<YagnaApiClient>>(
        &worker_name,
        api_client,
    );
}

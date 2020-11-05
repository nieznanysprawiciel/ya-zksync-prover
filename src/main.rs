use anyhow::anyhow;
use chrono::Utc;
use futures::prelude::*;
use std::ops::Add;
use structopt::StructOpt;

use std::time::Duration;
use url::Url;
use ya_agreement_utils::{constraints, ConstraintKey, Constraints};
use ya_client::web::WebClient;
use yarapi::requestor::Image;
use yarapi::rest::{self, Activity};
use zksync_prover::{client as zksync, ApiClient};

const PACKAGE: &str = "{TODO package}";

async fn create_agreement(market: rest::Market, subnet: &str) -> anyhow::Result<rest::Agreement> {
    let deadline = Utc::now().add(chrono::Duration::minutes(15));
    let ts = deadline.timestamp_millis();
    let props = serde_json::json!({
        "golem.node.id.name": "zk-sync-node",
        "golem.node.debug.subnet": subnet,
        "golem.srv.comp.task_package": PACKAGE,
        "golem.srv.comp.expiration": ts
    });

    let constraints = constraints![
        "golem.runtime.name" == Image::GVMKit((0, 1, 0).into()).runtime_name(),
        "golem.node.debug.subnet" == subnet
    ]
    .to_string();

    let subscrption = market.subscribe(&props, &constraints).await?;

    let proposals = subscrption.proposals();
    futures::pin_mut!(proposals);
    while let Some(proposal) = proposals.try_next().await? {
        log::info!(
            "got proposal: {} -- from: {}, draft: {:?}",
            proposal.id(),
            proposal.issuer_id(),
            proposal.state()
        );
        if proposal.is_response() {
            let agreement = proposal.create_agreement(deadline).await?;
            log::info!("created agreement {}", agreement.id());
            if let Err(e) = agreement.confirm().await {
                log::error!("wait_for_approval failed: {:?}", e);
                continue;
            }
            return Ok(agreement);
        }
        let id = proposal.counter_proposal(&props, &constraints).await?;
        log::info!("got: {}", id);
    }
    unimplemented!()
}

#[derive(StructOpt)]
struct Args {
    #[structopt(long, default_value = "devnet-alpha.2")]
    subnet: String,
    #[structopt(long, env = "YAGNA_APPKEY")]
    appkey: String,
    #[structopt(long, env)]
    server_api_url: String,
}

#[actix_rt::main]
pub async fn main() -> anyhow::Result<()> {
    let args = Args::from_args();
    std::env::set_var("RUST_LOG", "info,yarapi::drop=debug");
    env_logger::init();

    let server_api_url: Url = args.server_api_url.parse()?;
    let zksync_client =
        zksync::ApiClient::new(&server_api_url, "yagna-node", Duration::from_secs(69));

    let client = WebClient::with_token(&args.appkey);
    let session = rest::Session::with_client(client.clone());

    session
        .with(async {
            let agreement = create_agreement(session.market()?, &args.subnet).await?;
            let activity = session.create_activity(&agreement).await?;

            let prover_id = zksync_client.register_prover(0)?;

            // TODO: run zksync in loop here
            prove_block(zksync_client.clone()).await?;

            zksync_client.prover_stopped(prover_id)?;
            activity.destroy().await?;

            Ok::<_, anyhow::Error>(())
        })
        .await
        .unwrap_or_else(|| anyhow::bail!("ctrl-c caught"))
}

async fn prove_block(zksync_client: zksync::ApiClient) -> anyhow::Result<()> {
    // TODO: Set block size
    let block_size = 0;

    let (block_id, job_id) = zksync_client
        .block_to_prove(block_size)
        .map_err(|e| anyhow!("Failed to download block to prove. Error: {}", e))?
        .ok_or(anyhow!("Failed to find block of size {}", block_size))?;

    let instance = zksync_client
        .prover_data(block_id)
        .map_err(|e| anyhow!("Couldn't get data for block '{}'. Error: {}", block_id, e))?;

    Ok(())
}

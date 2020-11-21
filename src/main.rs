mod prover_runner;
mod transfer;
mod zksync_client;

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
use zksync_client::ZksyncClient;

use crate::prover_runner::prove_block;
use crate::transfer::execute_commands;
use std::sync::Arc;

const PACKAGE: &str =
    "hash:sha3:a58da3b594eb9d0519f09d31d5b6e1576c41c22ff8b9ee4b2b04119c:http://yacn.dev.golem.network:8000/docker-ya-zksync-prover-0.1-928c7db826.gvmi";

async fn create_agreement(market: rest::Market, subnet: &str) -> anyhow::Result<rest::Agreement> {
    let deadline = Utc::now().add(chrono::Duration::minutes(25));
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

    let subscription = market.subscribe(&props, &constraints).await?;
    log::info!("Created subscription [{}]", subscription.id().as_ref());

    let proposals = subscription.proposals();
    futures::pin_mut!(proposals);
    while let Some(proposal) = proposals.try_next().await? {
        log::info!(
            "Got proposal: {} -- from: {}, draft: {:?}",
            proposal.id(),
            proposal.issuer_id(),
            proposal.state()
        );
        if proposal.is_response() {
            let agreement = proposal.create_agreement(deadline).await?;
            log::info!("Created agreement {}", agreement.id());
            if let Err(e) = agreement.confirm().await {
                log::error!("wait_for_approval failed: {:?}", e);
                continue;
            }
            return Ok(agreement);
        }
        let id = proposal.counter_proposal(&props, &constraints).await?;
        log::info!("Got: {}", id);
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
    dotenv::dotenv().ok();

    let args = Args::from_args();
    std::env::set_var("RUST_LOG", "info,yarapi::drop=debug");
    env_logger::init();

    let server_api_url: Url = args.server_api_url.parse()?;
    let zksync_client = ZksyncClient::new(&server_api_url, "yagna-node-1", Duration::from_secs(69));

    let client = WebClient::with_token(&args.appkey);
    let session = rest::Session::with_client(client.clone());

    session
        .with(async {
            let agreement = create_agreement(session.market()?, &args.subnet).await?;

            log::info!("Registering prover..");
            let prover_id = zksync_client.register_prover(0).await?;
            log::info!("Registered prover under id [{}].", prover_id);

            let activity = Arc::new(session.create_activity(&agreement).await?);

            match execute_commands(
                activity.clone(),
                vec![
                    rest::ExeScriptCommand::Deploy {},
                    rest::ExeScriptCommand::Start { args: vec![] },
                ],
            )
            .await
            {
                Ok(_) => {
                    // TODO: run zksync in loop here
                    prove_block(zksync_client.clone(), activity.clone())
                        .await
                        .map_err(|e| log::error!("{}", e))
                        .ok();
                }
                Err(e) => log::error!("Failed to initialize task on yagna. Error: {}.", e),
            };

            log::info!("Destroying activity..");
            activity
                .destroy()
                .await
                .map_err(|e| log::error!("Can't destroy activity. Error: {}", e))
                .ok();

            log::info!("Stopping prover on zksync server..");
            zksync_client
                .prover_stopped(prover_id)
                .await
                .map_err(|e| log::error!("Failed to unregister prover on server. Error: {}", e))
                .ok();

            Ok::<_, anyhow::Error>(())
        })
        .await
        .unwrap_or_else(|| anyhow::bail!("ctrl-c caught"))
}

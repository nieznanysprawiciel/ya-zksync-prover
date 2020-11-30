mod prover_runner;
mod transfer;
mod zksync_client;

use anyhow::anyhow;
use chrono::Utc;
use futures::prelude::*;
use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;
use structopt::StructOpt;
use url::Url;

use ya_client::web::WebClient;
use yarapi::requestor::Image;
use yarapi::rest::{self, Activity};
use yarapi::ya_agreement_utils::{constraints, ConstraintKey, Constraints};
use zksync_client::ZksyncClient;

use crate::prover_runner::prove_block;
use crate::transfer::execute_commands;

const PACKAGE: &str =
    "hash:sha3:0bf9efb4822c5cfd5606e62698e1edac1951f1973d0d944ca1ad5f07:http://yacn.dev.golem.network:8000/ya-zksync-prover-0.2";

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
        "golem.runtime.name" == Image::GVMKit((0, 2, 3).into()).runtime_name(),
        "golem.node.debug.subnet" == subnet
    ]
    .to_string();

    let subscription = market.subscribe(&props, &constraints).await?;
    log::info!("Created subscription [{}]", subscription.id().as_ref());

    let proposals = subscription.proposals();
    futures::pin_mut!(proposals);
    while let Some(proposal) = proposals.try_next().await? {
        log::info!(
            "Got proposal: {} -- from: {}, state: {:?}",
            proposal.id(),
            proposal.issuer_id(),
            proposal.state()
        );
        if proposal.is_response() {
            let agreement = proposal.create_agreement(deadline).await?;
            if let Err(e) = agreement.confirm().await {
                log::error!("wait_for_approval failed: {:?}", e);
                continue;
            }

            // TODO: Use AgreementView.
            let name = agreement
                .content()
                .await?
                .offer
                .properties
                .pointer("/golem.node.id.name")
                .map(|value| value.as_str().map(|name| name.to_string()))
                .flatten()
                .ok_or(anyhow!("Can't find node name in Agreement"))?;

            log::info!("Created agreement [{}] with '{}'", agreement.id(), name);
            return Ok(agreement);
        }
        proposal
            .counter_proposal(&props, &constraints)
            .await
            .map_err(|e| log::warn!("Failed to counter Proposal. Error: {}", e))
            .ok();
    }
    unimplemented!()
}

#[derive(StructOpt)]
struct Args {
    #[structopt(long, env, default_value = "devnet-alpha.3")]
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
    std::env::set_var("RUST_LOG", "info");
    env_logger::builder()
        .filter_module("yarapi::drop", log::LevelFilter::Off)
        .filter_module("ya_service_bus::connection", log::LevelFilter::Off)
        .filter_module("ya_service_bus::remote_router", log::LevelFilter::Off)
        .init();

    log::info!("Using subnet: {}", &args.subnet);

    let server_api_url: Url = args.server_api_url.parse()?;
    let zksync_client = ZksyncClient::new(&server_api_url, "yagna-node-1", Duration::from_secs(69));

    let client = WebClient::with_token(&args.appkey);
    let session = rest::Session::with_client(client.clone());

    let agreement = create_agreement(session.market()?, &args.subnet).await?;

    log::info!("Registering prover..");
    let prover_id = zksync_client.register_prover(0).await?;
    log::info!("Registered prover under id [{}].", prover_id);

    let activity = Arc::new(session.create_activity(&agreement).await?);

    session
        .with(async {
            log::info!("Deploying image and starting ExeUnit...");
            if let Err(e) = execute_commands(
                activity.clone(),
                vec![
                    rest::ExeScriptCommand::Deploy {},
                    rest::ExeScriptCommand::Start { args: vec![] },
                ],
            )
            .await
            {
                log::error!("Failed to initialize yagna task. Error: {}.", e);
                return Ok(());
            };

            log::info!("Image deployed. ExeUnit started.");

            loop {
                match prove_block(zksync_client.clone(), activity.clone())
                    .await
                    .map_err(|e| log::warn!("{}", e))
                {
                    Err(_) => tokio::time::delay_for(Duration::from_secs(10)).await,
                    Ok(()) => (),
                }
            }
        })
        .await
        .unwrap_or_else(|| anyhow::bail!("ctrl-c caught"))
        .map_err(|e| log::info!("{}", e))
        .ok();

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

    Ok(())
}

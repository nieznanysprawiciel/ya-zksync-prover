mod prover_runner;
mod zksync_client;

use chrono::{DateTime, Utc};
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
use ya_client_model::market::NewDemand;

const PACKAGE: &str =
    "hash:sha3:b491514aa88dc7f79ed461358cf9ea9c63775da591312f2f1a1dc43d:http://yacn.dev.golem.network:8000/ya-zksync-prover-0.2.3";

pub fn create_demand(deadline: DateTime<Utc>, subnet: &str) -> NewDemand {
    log::info!("Using subnet: {}", subnet);

    let ts = deadline.timestamp_millis();
    let properties = serde_json::json!({
        "golem.node.id.name": "zk-sync-node",
        "golem.node.debug.subnet": subnet,
        "golem.srv.comp.task_package": PACKAGE,
        "golem.srv.comp.expiration": ts
    });

    let constraints = constraints![
        "golem.runtime.name" == Image::GVMKit((0, 2, 3).into()).runtime_name(),
        "golem.node.debug.subnet" == subnet,
        "golem.inf.mem.gib" > 16,
        "golem.inf.storage.gib" > 1.0
    ]
    .to_string();

    NewDemand {
        properties,
        constraints,
    }
}

#[derive(StructOpt)]
struct Args {
    #[structopt(long, env, default_value = "community.3")]
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

    let server_api_url: Url = args.server_api_url.parse()?;
    let zksync_client = ZksyncClient::new(&server_api_url, "yagna-node-1", Duration::from_secs(69));

    let client = WebClient::with_token(&args.appkey);
    let session = rest::Session::with_client(client.clone());
    let market = session.market()?;

    let deadline = Utc::now().add(chrono::Duration::minutes(25));
    let demand = create_demand(deadline, &args.subnet);

    let subscription = market.subscribe_demand(demand.clone()).await?;
    log::info!("Created subscription [{}]", subscription.id().as_ref());

    log::info!("Registering prover..");
    let prover_id = zksync_client.register_prover(0).await?;
    log::info!("Registered prover under id [{}].", prover_id);

    let agreements = subscription
        .negotiate_agreements(demand, 1, deadline)
        .await?;
    let activity = Arc::new(session.create_activity(&agreements[0]).await?);

    session
        .with(async {
            log::info!("Deploying image and starting ExeUnit...");
            if let Err(e) = activity
                .execute_commands(vec![
                    rest::ExeScriptCommand::Deploy {},
                    rest::ExeScriptCommand::Start { args: vec![] },
                ])
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

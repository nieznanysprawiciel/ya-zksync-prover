use anyhow::{anyhow, bail};
use futures::future::ready;
use futures::StreamExt;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use crate::zksync_client::ZksyncClient;
use ya_client_model::activity::{CommandOutput, RuntimeEventKind};
use yarapi::rest::activity::DefaultActivity;
use yarapi::rest::streaming::{ResultStream, StreamingActivity};
use yarapi::rest::Transfers;
use zksync_crypto::proof::EncodedProofPlonk;

#[derive(Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub block_id: i64,
    pub job_id: i32,
    pub block_size: usize,
}

pub async fn prove_block(
    zksync_client: Arc<ZksyncClient>,
    activity: Arc<DefaultActivity>,
) -> anyhow::Result<()> {
    let block = ask_for_block(zksync_client.clone()).await?;

    log::info!(
        "Got block '{}' of size '{}' to prove. Job id: '{}'.",
        &block.block_id,
        &block.block_size,
        &block.job_id
    );

    activity
        .send_json(&PathBuf::from_str("/blocks/job-info.json")?, &block)
        .await
        .map_err(|e| anyhow!("Transferring block info: {}", e))?;

    // TODO: Save job info on disk for debugging.
    fs::create_dir_all("blocks")?;
    let job_file = PathBuf::from(format!("blocks/job-info-{}.json", block.job_id));
    save(&job_file, &block).map_err(|e| anyhow!("Failed to debug job info. {}", e))?;

    // This line will set last job info parameters in blocks directory. You can run docker container locally
    // in workdir and it should work the same as on provider.
    fs::copy(&job_file, "blocks/job-info.json").ok();

    // TODO: Modify zksync to return ProverData here.
    // TODO: We shouldn't download block here. Generate address and command ExeUnit to download this data.
    let data = zksync_client
        .prover_data(block.block_id)
        .await
        .map_err(|e| {
            anyhow!(
                "Couldn't get data for block '{}'. Error: {}",
                &block.block_id,
                e
            )
        })?;

    // TODO: Save block on disk for debugging.
    save(
        &PathBuf::from(format!("blocks/block-{}.json", block.block_id)),
        &data,
    )
    .map_err(|e| anyhow!("Failed to debug save block. {}", e))?;

    // TODO: Remove downloading in future. Provider ExeUnit will do it.
    log::info!("Downloaded prover data. Uploading data to Provider...");
    let block_remote_path = PathBuf::from(format!("/blocks/block-{}.json", block.block_id));
    activity.send_json(&block_remote_path, &data).await?;

    log::info!("Block uploaded. Running prover on remote yagna node...");
    run_yagna_prover(activity.clone())
        .await
        .map_err(|e| anyhow!("Failed to run prover on remote node. Error: {}", e))?;

    // Notify server, that we are computing proof for block.
    zksync_client.working_on(block.job_id).await.map_err(|e| {
        anyhow!(
            "Working on job '{}'. Failed to notify zksync server. Error: {}",
            block.job_id,
            e
        )
    })?;

    log::info!("Proof for block generated. Downloading...");

    let proof_path = PathBuf::from(format!("/proofs/proof-{}.json", &block.block_id));
    let verified_proof: EncodedProofPlonk = activity.download_json(&proof_path).await?;

    log::info!("Proof downloaded. Publishing proof on server...");

    fs::create_dir_all("proofs").ok();
    save(
        &PathBuf::from(format!("proofs/proof-{}.json", &block.block_id)),
        &verified_proof,
    )
    .map_err(|e| log::warn!("Failed to debug save proof. {}", e))
    .ok();

    zksync_client
        .publish(block.block_id, verified_proof)
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to publish proof for block '{}' and job '{}'. Error: {}",
                block.block_id,
                block.job_id,
                e
            )
        })?;

    log::info!("Block '{}' published.", block.block_id);
    Ok(())
}

async fn ask_for_block(zksync_client: Arc<ZksyncClient>) -> anyhow::Result<BlockInfo> {
    // TODO: Make configurable
    let supported_sizes: Vec<usize> = vec![6, 30, 74, 150, 320, 630];

    // Try ask server for different sizes of blocks.
    for block_size in supported_sizes.iter().cloned() {
        let info = zksync_client
            .block_to_prove(block_size)
            .await
            .map_err(|e| anyhow!("Failed to download block to prove. Error: {}", e))?;

        if let Some((block_id, job_id)) = info {
            return Ok(BlockInfo {
                block_id,
                block_size,
                job_id,
            });
        } else {
            log::debug!(
                "Block of size {} not found. Checking other possible sizes",
                block_size
            );
        }
    }
    bail!("Checked all possible block sizes and didn't find any.")
}

async fn run_yagna_prover(activity: Arc<DefaultActivity>) -> anyhow::Result<()> {
    let bar_max: u64 = 1644;
    let bar = ProgressBar::new(bar_max);

    bar.inc(0);

    let batch = activity
        .run_streaming("/bin/yagna-prover", vec!["ya-prover".to_string()])
        .await?
        .debug(".debug")?;
    batch
        .stream()
        .await?
        .forward_to_file(
            &PathBuf::from("stdout-output.txt"),
            &PathBuf::from("stderr-output.txt"),
        )?
        .inspect(|event| match &event.kind {
            RuntimeEventKind::StdOut(output) => bar.inc(match output {
                CommandOutput::Str(text) => text.len(),
                CommandOutput::Bin(vec) => vec.len(),
            } as u64),
            _ => (),
        })
        .take_while(|event| {
            ready(match &event.kind {
                RuntimeEventKind::Finished {
                    return_code,
                    message,
                } => {
                    let no_msg = "".to_string();
                    log::info!(
                        "ExeUnit finished proving with code {}, and message: {}",
                        return_code,
                        message.as_ref().unwrap_or(&no_msg)
                    );
                    false
                }
                _ => true,
            })
        })
        .for_each(|_| ready(()))
        .await;
    batch.wait_for_finish().await?;

    bar.set_position(bar_max);
    bar.finish_and_clear();
    Ok(())
}

// Saving blocks for debugging.
fn save<T: Sized + Serialize>(data_path: &Path, data: &T) -> anyhow::Result<()> {
    use std::fs::File;

    let file = File::create(&data_path).map_err(|e| {
        anyhow!(
            "Can't open data file [{}]. Error: {}",
            data_path.display(),
            e
        )
    })?;

    serde_json::to_writer(file, &data).map_err(|e| {
        anyhow!(
            "Failed to serialize data to file: {}. Error: {}",
            data_path.display(),
            e
        )
    })?;
    Ok(())
}

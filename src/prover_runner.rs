use anyhow::{anyhow, bail};
use std::path::PathBuf;
use std::sync::Arc;

use crate::zksync_client::ZksyncClient;

use zksync_crypto::proof::EncodedProofPlonk;

#[derive(Clone)]
pub struct BlockInfo {
    pub block_id: i64,
    pub job_id: i32,
    pub block_size: usize,
}

pub async fn prove_block(zksync_client: Arc<ZksyncClient>) -> anyhow::Result<()> {
    let block = ask_for_block(zksync_client.clone()).await?;

    log::info!(
        "Got block '{}' of size '{}' to prove. Job id: '{}'.",
        &block.block_id,
        &block.block_size,
        &block.job_id
    );

    // TODO: Consider calling this function later, after yagna provider starts working on task.
    zksync_client.working_on(block.job_id).await.map_err(|e| {
        anyhow!(
            "Working on job '{}'. Failed to notify zksync server. Error: {}",
            block.job_id,
            e
        )
    })?;

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

    // TODO: Remove donwloading in future. Provider ExeUnit will do it.
    use std::fs::File;

    let data_path = PathBuf::from("prover_data.json");
    let file = File::open(&data_path).map_err(|e| {
        anyhow!(
            "Can't open data file [{}]. Error: {}",
            data_path.display(),
            e
        )
    })?;

    serde_json::to_writer(file, &data).map_err(|e| {
        anyhow!(
            "Failed to serialize block {}. Error: {}",
            &block.block_id,
            e
        )
    })?;

    log::info!("Downloaded prover data. Uploading data to Provider.");

    // TODO: Run prover on provider node.
    let verified_proof = EncodedProofPlonk::default();

    log::info!("Block verified. Publishing proof on server...");

    // TODO: Download proof from provider.
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
            log::info!(
                "Block of size {} not found. Checking other possible sizes",
                block_size
            );
        }
    }
    bail!("Checked all possible block sizes and didn't find anyone.")
}

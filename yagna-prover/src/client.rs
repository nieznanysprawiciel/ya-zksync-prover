use anyhow::anyhow;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{self};

use zksync_crypto::proof::EncodedProofPlonk;
use zksync_prover::ApiClient;
use zksync_prover_utils::prover_data::ProverData;

#[derive(Debug, Clone)]
pub struct YagnaApiClient {
    finish: Arc<AtomicBool>,
}

impl YagnaApiClient {
    pub fn new(_base_url: &Url, _worker: &str, _req_server_timeout: time::Duration) -> Self {
        YagnaApiClient {
            finish: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub block_id: i64,
    pub job_id: i32,
    pub block_size: usize,
}

impl ApiClient for YagnaApiClient {
    fn block_to_prove(&self, block_size: usize) -> Result<Option<(i64, i32)>, anyhow::Error> {
        // Download block info from disk.
        // Yagna Requestor should upload json file for us.
        let job_path = blocks_info_dir().join("job-info.json");
        let json_file = File::open(&job_path).map_err(|e| {
            anyhow!(
                "Can't open job info file [{}] to deserialize. Error: {}",
                &job_path.display(),
                e
            )
        })?;
        let info: BlockInfo = serde_json::from_reader(json_file).map_err(|e| {
            anyhow!(
                "Failed to deserialize info for block of size {}. Error: {}",
                block_size,
                e
            )
        })?;

        // plonk_step_by_step_prover will try with all supported sizes.
        // So we shouldn't confuse him by returning block of different size, then he expected.
        if info.block_size == block_size {
            Ok(Some((info.block_id, info.job_id)))
        } else {
            Ok(None)
        }
    }

    fn working_on(&self, _job_id: i32) -> Result<(), anyhow::Error> {
        // This will be responsibility of Yagna Requestor.
        Ok(())
    }

    fn prover_data(&self, block: i64) -> Result<ProverData, anyhow::Error> {
        if self.finish.load(Ordering::SeqCst) {
            log::info!(
                "Stopping.. This prover computes only one proof per execution. \
                 This is not an error but normal behavior."
            );
            std::process::exit(0);
        }

        // Yagna Requestor will command ExeUnit to download block and place in our directories.
        let block_path = blocks_info_dir().join(format!("block-{}.json", block));

        let json_file = File::open(&block_path).map_err(|e| {
            anyhow!(
                "Can't open block file [{}] to deserialize. Error: {}",
                &block_path.display(),
                e
            )
        })?;
        let prover_data: ProverData = serde_json::from_reader(json_file)
            .map_err(|e| anyhow!("Failed to deserialize block {}. Error: {}", block, e))?;

        Ok(prover_data)
    }

    fn publish(&self, block: i64, proof: EncodedProofPlonk) -> Result<(), anyhow::Error> {
        // Serialize proof and save on disk.
        // Yagna Requestor will download it from expected location and send to zksync server.
        let proof_path = proofs_info_dir().join(format!("proof-{}.json", block));
        let file = File::create(&proof_path).map_err(|e| {
            anyhow!(
                "Can't open proof file [{}]. Error: {}",
                proof_path.display(),
                e
            )
        })?;

        serde_json::to_writer(file, &proof)
            .map_err(|e| anyhow!("Failed to serialize block {}. Error: {}", block, e))?;

        // We run only single proof. Yagna Requestor will run VM multiple times.
        // TODO: Implement it better on Requestor side.
        self.finish.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn prover_stopped(&self, _prover_run_id: i32) -> Result<(), anyhow::Error> {
        // Yagna Reuestor will notify server, after prover will stop.
        Ok(())
    }

    fn register_prover(&self, _block_size: usize) -> Result<i32, anyhow::Error> {
        // Prover doesn't see external world so it can be anything.
        Ok(32)
    }
}

pub fn proofs_info_dir() -> PathBuf {
    PathBuf::from("/proofs/")
}

pub fn blocks_info_dir() -> PathBuf {
    PathBuf::from("/blocks/")
}

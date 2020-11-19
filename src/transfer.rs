use anyhow::*;
use futures::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::TryStreamExt;
use serde::de::DeserializeOwned;
use std::io::BufReader;
use yarapi::rest::activity::DefaultActivity;
use yarapi::rest::{Activity, ExeScriptCommand, RunningBatch};

pub struct Transfers {
    activity: Arc<DefaultActivity>,
}

impl Transfers {
    pub fn new(activity: Arc<DefaultActivity>) -> Transfers {
        Transfers { activity }
    }

    #[allow(dead_code)]
    pub async fn send_file(&self, src: String, dest: &Path) -> Result<(), Error> {
        transfer(self.activity.clone(), &PathBuf::from(src), dest).await
    }

    pub async fn send_json<T: Serialize>(
        &self,
        dest: &Path,
        to_serialize: &T,
    ) -> Result<(), Error> {
        let file = tempfile::NamedTempFile::new()
            .map_err(|e| anyhow!("Failed to create temporary file. Error: {}", e))?;
        let file_path = file.path().to_path_buf();
        serde_json::to_writer(file, to_serialize)
            .map_err(|e| anyhow!("Failed to serialize object to temp file. {}", e))?;

        transfer(self.activity.clone(), &file_path, dest).await
    }

    #[allow(dead_code)]
    pub async fn download_file(&self, src: &Path, dest: &Path) -> Result<(), Error> {
        transfer(self.activity.clone(), src, dest).await
    }

    pub async fn download_json<T: DeserializeOwned>(&self, src: &Path) -> Result<T, Error> {
        let file = tempfile::NamedTempFile::new()
            .map_err(|e| anyhow!("Failed to create temporary file. Error: {}", e))?;

        transfer(self.activity.clone(), src, file.path()).await?;

        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .map_err(|e| anyhow!("Failed to deserialize object from temp file. {}", e))
    }
}

async fn transfer(activity: Arc<DefaultActivity>, src: &Path, dest: &Path) -> anyhow::Result<()> {
    let dest = dest
        .to_path_buf()
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("Can't convert [{}] to String. {:?}", dest.display(), e))?;

    let src = src
        .to_path_buf()
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("Can't convert [{}] to String. {:?}", src.display(), e))?;

    let commands = vec![ExeScriptCommand::Transfer {
        from: src.clone(),
        to: dest.clone(),
        args: Default::default(),
    }];

    if let Err(e) = execute_commands(activity, commands).await {
        let message = anyhow!(
            "Error transferring file from [{}] to [{}]. Error: {}",
            &src,
            &dest,
            e
        );
        log::error!("{}", &message);
        return Err(message);
    }
    Ok(())
}

pub async fn execute_commands(
    activity: Arc<DefaultActivity>,
    commands: Vec<ExeScriptCommand>,
) -> anyhow::Result<()> {
    let batch = activity.exec(commands).await?;
    batch
        .events()
        .try_for_each(|event| {
            log::info!("event: {:?}", event);
            future::ok(())
        })
        .await
}

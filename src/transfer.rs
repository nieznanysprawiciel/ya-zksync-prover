use anyhow::*;
use futures::prelude::*;
use futures::TryStreamExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use url::Url;

use yarapi::rest::activity::DefaultActivity;
use yarapi::rest::{Activity, ExeScriptCommand, RunningBatch};

pub struct Transfers {
    activity: Arc<DefaultActivity>,
}

impl Transfers {
    pub fn new(activity: Arc<DefaultActivity>) -> Transfers {
        Transfers { activity }
    }

    pub async fn send_file(&self, src: &Path, dest: &Path) -> Result<(), Error> {
        let src = gftp::publish(&src).await?;
        let mut dest = Url::from_file_path(dest)
            .map_err(|_| anyhow!("Can't convert [{}] to url.", dest.display()))?;
        dest.set_scheme("container")
            .map_err(|_| anyhow!("Failed to set 'container' scheme for path [{}].", dest))?;

        transfer(self.activity.clone(), &src, &dest).await
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

        self.send_file(&file_path, dest).await
    }

    #[allow(dead_code)]
    pub async fn download_file(&self, src: &Path, dest: &Path) -> Result<(), Error> {
        let dest = gftp::open_for_upload(&dest).await?;
        let mut src = Url::from_file_path(src)
            .map_err(|_| anyhow!("Can't convert [{}] to url.", src.display()))?;
        src.set_scheme("container")
            .map_err(|_| anyhow!("Failed to set 'container' scheme for path [{}].", src))?;

        transfer(self.activity.clone(), &src, &dest).await
    }

    pub async fn download_json<T: DeserializeOwned>(&self, src: &Path) -> Result<T, Error> {
        let file = tempfile::NamedTempFile::new()
            .map_err(|e| anyhow!("Failed to create temporary file. Error: {}", e))?;

        self.download_file(src, file.path()).await?;

        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .map_err(|e| anyhow!("Failed to deserialize object from temp file. {}", e))
    }
}

async fn transfer(activity: Arc<DefaultActivity>, src: &Url, dest: &Url) -> anyhow::Result<()> {
    let commands = vec![ExeScriptCommand::Transfer {
        from: src.clone().into_string(),
        to: dest.clone().into_string(),
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

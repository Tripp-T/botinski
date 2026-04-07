use {
    anyhow::{Context, bail},
    serde::{Serialize, de::DeserializeOwned},
    std::{
        io::{BufReader, Write},
        path::Path,
    },
};

enum FileHandler {
    Json,
    Toml,
}
impl FileHandler {
    pub fn from_path<P: AsRef<Path>>(p: P) -> anyhow::Result<Self> {
        let path = p.as_ref();
        let ext = path
            .extension()
            .context("Missing file extension")?
            .to_ascii_lowercase();
        if ext == "json" {
            Ok(Self::Json)
        } else if ext == "toml" {
            Ok(Self::Toml)
        } else {
            bail!("Unhandled file extension: {}", ext.to_string_lossy())
        }
    }
    pub fn read<T: DeserializeOwned, P: AsRef<Path>>(&self, path: P) -> anyhow::Result<T> {
        let path = path.as_ref();
        match self {
            Self::Json => {
                let file = std::fs::File::open(path)
                    .with_context(|| format!("failed to open file: {}", path.display()))?;
                let reader = BufReader::new(file);
                serde_json::from_reader(reader)
                    .with_context(|| format!("failed to parse JSON from: {}", path.display()))
            }
            Self::Toml => {
                let file_str = std::fs::read_to_string(path)
                    .with_context(|| format!("failed to read file: {}", path.display()))?;
                toml::from_str(&file_str)
                    .with_context(|| format!("failed to parse TOML from: {}", path.display()))
            }
        }
    }
    pub fn write<T: Serialize, P: AsRef<Path>>(&self, path: P, value: &T) -> anyhow::Result<()> {
        let data = match self {
            Self::Json => {
                serde_json::to_string_pretty(value).context("failed to serialize data to json")
            }
            Self::Toml => toml::to_string_pretty(value).context("failed to serialize data"),
        }?;
        let destination_path = path.as_ref();
        let tmp_path = destination_path.with_extension("tmp");
        let mut tmp_file = std::fs::File::create(&tmp_path).context("failed to open tmp file")?;
        tmp_file
            .write_all(data.as_bytes())
            .context("failed to write data")?;
        tmp_file.sync_all().context("failed to sync data write")?;
        std::fs::rename(tmp_path, destination_path).context("failed to write data")
    }
}

pub fn load_file<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> anyhow::Result<T> {
    FileHandler::from_path(&path)?.read(path)
}
pub fn write_file<T: Serialize, P: AsRef<Path>>(path: P, data: &T) -> anyhow::Result<()> {
    FileHandler::from_path(&path)?.write(path, data)
}

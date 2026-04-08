use anyhow::{Context, bail};
use std::path::Path;

static OUTPUT_DIR: &str = "./target/dist/";

static CSS_INPUT_FILE: &str = "./input.css";
static CSS_OUTPUT_FILE: &str = "./target/dist/output.css";

fn main() -> anyhow::Result<()> {
    clean_dist().context("Failed to cleanup old output dir")?;
    build_css().context("Failed to build CSS")?;
    copy_public().context("Failed to copy public assets")?;
    Ok(())
}

fn clean_dist() -> anyhow::Result<()> {
    match std::fs::remove_dir_all(OUTPUT_DIR) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).context("Failed to delete old output dir"),
    }
}

fn build_css() -> anyhow::Result<()> {
    let status = std::process::Command::new("tailwindcss")
        .args(["-i", CSS_INPUT_FILE])
        .args(["-o", CSS_OUTPUT_FILE])
        .status()
        .context("failed to spawn command")?;
    if !status.success() {
        bail!("Tailwind command failed with exit status: {status}");
    }
    Ok(())
}

fn copy_public() -> anyhow::Result<()> {
    let start_dir = Path::new("./public");
    let out_dir = Path::new(OUTPUT_DIR);

    fn copy_dir<P: AsRef<Path>>(start_dir: P, out_dir: P) -> anyhow::Result<()> {
        let start_dir = start_dir.as_ref();
        let out_dir = out_dir.as_ref();
        // Ensure the target directory exists before trying to copy into it
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("Failed to create directory: {out_dir:?}"))?;

        for entry in std::fs::read_dir(start_dir)
            .with_context(|| format!("Failed to read dir: {start_dir:?}"))?
        {
            let entry = entry.context("Failed to read directory entry")?;
            let entry_path = entry.path();

            let Some(entry_file_name) = entry_path.file_name() else {
                eprintln!("Encountered file entry with no file name: {entry_path:?}");
                continue;
            };

            let target_path = out_dir.join(entry_file_name);

            if entry_path.is_dir() {
                copy_dir(&entry_path, &target_path)?;
            } else {
                std::fs::copy(&entry_path, &target_path).with_context(|| {
                    format!("Failed to copy file from {entry_path:?} to {target_path:?}")
                })?;
            }
        }
        Ok(())
    }

    // Only copy if the public directory actually exists to prevent crashing if it's missing
    if start_dir.exists() {
        copy_dir(start_dir, out_dir)?;
    }

    Ok(())
}

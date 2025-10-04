use std::{
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use color_eyre::eyre::eyre;
use fs_err::File;
use parking_lot::Mutex;
use reqwest::header;
use zip::ZipArchive;

const AUTHOR_NAME: &str = "eigeen";
const REPO_NAME: &str = "mhws-tex-decompressor";

#[allow(dead_code)]
/// Release information
#[derive(Clone, Debug, Default)]
pub struct Release {
    pub name: String,
    pub version: String,
    pub date: String,
    pub body: Option<String>,
    pub asset: Option<ReleaseAsset>,
}

#[allow(dead_code)]
/// Release asset information
#[derive(Clone, Debug, Default)]
pub struct ReleaseAsset {
    pub download_url: String,
    pub name: String,
}

#[derive(Default)]
struct State {
    release: Option<Release>,
    replace_file: Option<PathBuf>,
}

pub struct Updater {
    client: reqwest::Client,
    state: Mutex<State>,
}

impl Updater {
    fn new() -> Self {
        let headers = header::HeaderMap::from_iter([(
            header::HeaderName::from_static("user-agent"),
            header::HeaderValue::from_str(&format!("{}/{}", REPO_NAME, env!("CARGO_PKG_VERSION")))
                .unwrap(),
        )]);

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .default_headers(headers)
            .build()
            .unwrap();

        Self {
            client,
            state: Mutex::new(State::default()),
        }
    }

    pub fn get() -> &'static Self {
        static INSTANCE: OnceLock<Updater> = OnceLock::new();
        INSTANCE.get_or_init(Updater::new)
    }

    pub async fn check_update(&self) -> color_eyre::Result<Option<Release>> {
        // get releases information
        let resp = self
            .client
            .get(format!(
                "https://api.github.com/repos/{AUTHOR_NAME}/{REPO_NAME}/releases"
            ))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(eyre!(
                "Failed to get release information: {}",
                resp.status()
            ));
        }

        let release_info: Vec<serde_json::Value> = resp.json().await?;

        if release_info.is_empty() {
            return Err(eyre!("No release information found."));
        }

        let latest_release = release_info.into_iter().next().unwrap();

        // check if update is available
        let tag = latest_release["tag_name"]
            .as_str()
            .expect("tag_name is not a string");
        let latest_version = semver::Version::parse(tag.trim_start_matches('v'))?;
        let current_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

        if latest_version <= current_version {
            return Ok(None);
        }

        let expected_asset_prefix = Self::expected_asset_prefix();

        // get release assets
        let asset = latest_release["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|asset| {
                let name = asset["name"].as_str()?;
                if !name.starts_with(&expected_asset_prefix) {
                    return None;
                }

                let download_url = asset["browser_download_url"].as_str()?;
                Some(ReleaseAsset {
                    download_url: download_url.to_string(),
                    name: name.to_string(),
                })
            });

        // build release object
        let release = Release {
            name: latest_release["name"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            version: latest_version.to_string(),
            date: latest_release["published_at"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            body: latest_release["body"].as_str().map(|body| body.to_string()),
            asset,
        };

        self.state.lock().release = Some(release.clone());

        Ok(Some(release))
    }

    /// Download update from URL, and save it to a temporary file.
    ///
    /// The `on_progress` function is called with the current and total bytes downloaded.
    ///
    /// Returns the path of the downloaded file.
    pub async fn download_update<F>(&self, on_progress: F) -> color_eyre::Result<PathBuf>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        let url = {
            let state = self.state.lock();
            let Some(release) = state.release.as_ref() else {
                return Err(eyre!(
                    "No release information found. Check for updates first."
                ));
            };
            let Some(asset) = release.asset.as_ref() else {
                return Err(eyre!("No release asset found."));
            };
            asset.download_url.clone()
        };

        let mut resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(eyre!("Failed to download update: {}", resp.status()));
        }

        let Some(total_size) = resp.content_length() else {
            return Err(eyre!("Failed to get content length of update file."));
        };

        // make temp file
        let mut file = tempfile::Builder::new()
            .prefix(concat!(env!("CARGO_PKG_NAME"), "-update"))
            .tempfile_in("./")?;
        let file_path = file.path().to_path_buf();

        {
            let mut writer = std::io::BufWriter::new(&mut file);
            let mut downloaded = 0;

            while let Some(chunk) = resp.chunk().await? {
                writer.write_all(&chunk)?;
                downloaded += chunk.len() as u64;
                on_progress(downloaded, total_size);
            }
            writer.flush()?;
        }

        // extract file if it's a zip archive
        // read and check the magic
        let mut magic = [0; 4];
        file.seek(std::io::SeekFrom::Start(0))?;
        file.read_exact(&mut magic)?;

        if magic == [0x50, 0x4b, 0x03, 0x04] {
            // zip archive
            let extracted_path = self.extract_zip_archive(&file_path)?;
            self.state.lock().replace_file = Some(extracted_path.clone());
            Ok(extracted_path)
        } else {
            self.state.lock().replace_file = Some(file_path.clone());
            Ok(file_path)
        }
    }

    /// Replace the current executable with the downloaded update.
    pub fn perform_update_and_close(&self) -> color_eyre::Result<()> {
        let replace_path = {
            let state = self.state.lock();
            let Some(replace_file) = state.replace_file.as_ref() else {
                return Err(eyre!("No update file found."));
            };
            replace_file.clone()
        };

        if !replace_path.exists() {
            return Err(eyre!(
                "Update file does not exist: {}",
                replace_path.display()
            ));
        }

        self_replace::self_replace(&replace_path)?;
        let _ = fs_err::remove_file(replace_path);
        std::process::exit(0);
    }

    fn expected_asset_prefix() -> String {
        format!("{}-{}", REPO_NAME, env!("COMPILED_TARGET_TRIPLE"))
    }

    /// Extract zip archive and find the executable file.
    ///
    /// Returns the path to the extracted executable file that starts with `CARGO_PKG_NAME`.
    fn extract_zip_archive(&self, zip_path: &Path) -> color_eyre::Result<PathBuf> {
        let file = File::open(zip_path)?;
        let mut archive = ZipArchive::new(file)?;

        // create a temporary directory for extraction
        let extract_dir = tempfile::Builder::new()
            .prefix("mhws-update-")
            .tempdir_in("./")?;

        // extract all files
        archive.extract(extract_dir.path())?;

        // find the executable file that starts with CARGO_PKG_NAME
        let pkg_name = env!("CARGO_PKG_NAME");
        let mut found_files = Vec::new();

        for entry in fs_err::read_dir(extract_dir.path())? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if file_name_str.starts_with(pkg_name) {
                found_files.push(entry.path());
            }
        }

        match found_files.len() {
            0 => Err(eyre!(
                "No executable file found in the zip archive that starts with '{}'",
                pkg_name
            )),
            1 => {
                // move the file to prevent it from being deleted when extract_dir is dropped
                let found_file = &found_files[0];
                let new_file_name = format!("{}-update", found_file.file_stem().unwrap().display());
                let new_path = extract_dir.path().parent().unwrap().join(new_file_name);
                fs_err::rename(found_file, &new_path)?;

                Ok(new_path)
            }
            _ => Err(eyre!(
                "Multiple files found in the zip archive that start with '{}': {:?}, and we don't know which one to use.",
                pkg_name,
                found_files
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expected_asset_prefix() {
        assert_eq!(
            Updater::expected_asset_prefix(),
            format!("{}-{}", REPO_NAME, env!("COMPILED_TARGET_TRIPLE"))
        );

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        assert_eq!(
            Updater::expected_asset_prefix(),
            "mhws-tex-decompressor-x86_64-pc-windows-msvc"
        );

        eprintln!(
            "expected_asset_prefix: {}",
            Updater::expected_asset_prefix()
        )
    }
}

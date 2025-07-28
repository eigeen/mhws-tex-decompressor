//! Extended metadata for generated pak files.

use std::io::{self, Read, Write};

use ree_pak_core::{
    filename::{FileNameExt, FileNameFull},
    pak::PakArchive,
    read::archive::PakArchiveReader,
    write::{FileOptions, PakWriter},
};
use serde::{Deserialize, Serialize};

const METADATA_KEY: &str = "__TEX_DECOMPRESSOR_METADATA__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PakMetadata {
    version: u32,
    is_full_package: bool,
}

impl PakMetadata {
    pub fn new(is_full_package: bool) -> Self {
        Self {
            version: 1,
            is_full_package,
        }
    }

    pub fn is_full_package(&self) -> bool {
        self.is_full_package
    }

    pub fn from_pak_archive<R>(
        reader: R,
        pak_archive: &PakArchive,
    ) -> color_eyre::Result<Option<Self>>
    where
        R: io::Read + io::Seek,
    {
        let key_name = FileNameFull::new(METADATA_KEY);
        let entry = pak_archive
            .entries()
            .iter()
            .find(|entry| entry.hash() == key_name.hash_mixed());

        if let Some(entry) = entry {
            // read file
            let mut archive_reader = PakArchiveReader::new(reader, pak_archive);
            let mut entry_reader = archive_reader.owned_entry_reader(entry.clone())?;
            let mut buf = Vec::new();
            entry_reader.read_to_end(&mut buf)?;

            let metadata = serde_json::from_slice(&buf)?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    pub fn write_to_pak<W>(&self, pak_writer: &mut PakWriter<W>) -> color_eyre::Result<()>
    where
        W: io::Write + io::Seek,
    {
        let json_str = serde_json::to_string(self)?;
        let json_bytes = json_str.as_bytes();

        pak_writer.start_file(METADATA_KEY, FileOptions::default())?;
        pak_writer.write_all(json_bytes)?;

        Ok(())
    }
}

//! Chunk file name format
//!
//! File name structure:
//! - Base: re_chunk_XXX.pak
//! - Patch: re_chunk_XXX.pak.patch_XXX.pak
//! - Sub: re_chunk_XXX.pak.sub_XXX.pak
//! - Sub Patch: re_chunk_XXX.pak.sub_XXX.pak.patch_XXX.pak

use color_eyre::eyre;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkName {
    /// Major chunk ID (XXX in re_chunk_XXX.pak)
    pub major_id: u32,
    /// Patch number (XXX in .patch_XXX.pak)
    pub patch_id: Option<u32>,
    /// Sub chunk ID (XXX in .sub_XXX.pak)
    pub sub_id: Option<u32>,
    /// Patch number for sub chunk (YYY in .sub_XXX.pak.patch_YYY.pak)
    pub sub_patch_id: Option<u32>,
}

impl ChunkName {
    /// Create a new base chunk name (re_chunk_XXX.pak)
    pub fn new(major_id: u32) -> Self {
        Self {
            major_id,
            patch_id: None,
            sub_id: None,
            sub_patch_id: None,
        }
    }

    /// Create a chunk name from a string
    pub fn try_from_str(name: &str) -> color_eyre::Result<Self> {
        let dot_parts = name.split('.').collect::<Vec<&str>>();
        if dot_parts.len() < 2 || dot_parts.len() % 2 != 0 {
            return Err(eyre::eyre!(
                "Invalid chunk name with odd number of parts: {}",
                name
            ));
        }

        // every 2 parts is a component
        let components = dot_parts
            .chunks_exact(2)
            .map(|c| (c[0], c[1]))
            .collect::<Vec<(&str, &str)>>();
        // check if all parts have the correct extension
        if !components.iter().all(|(_, ext)| *ext == "pak") {
            return Err(eyre::eyre!(
                "Invalid chunk name with invalid extension: {}",
                name
            ));
        }

        let mut this = Self::new(0);

        for (name, _) in components.iter() {
            let component = Self::parse_component(name)?;
            match component {
                Component::Major(id) => this.major_id = id,
                Component::Sub(id) => this.sub_id = Some(id),
                Component::Patch(id) => {
                    if this.sub_id.is_some() {
                        this.sub_patch_id = Some(id);
                    } else {
                        this.patch_id = Some(id);
                    }
                }
            }
        }

        Ok(this)
    }

    fn parse_component(name: &str) -> color_eyre::Result<Component> {
        if name.starts_with("re_chunk_") {
            let major_id = name
                .strip_prefix("re_chunk_")
                .unwrap()
                .parse::<u32>()
                .map_err(|e| eyre::eyre!("Chunk name with invalid major ID: {}", e))?;
            Ok(Component::Major(major_id))
        } else if name.starts_with("patch_") {
            let patch_id = name
                .strip_prefix("patch_")
                .unwrap()
                .parse::<u32>()
                .map_err(|e| eyre::eyre!("Chunk name with invalid patch ID: {}", e))?;
            Ok(Component::Patch(patch_id))
        } else if name.starts_with("sub_") {
            let sub_id = name
                .strip_prefix("sub_")
                .unwrap()
                .parse::<u32>()
                .map_err(|e| eyre::eyre!("Chunk name with invalid sub ID: {}", e))?;
            Ok(Component::Sub(sub_id))
        } else {
            Err(eyre::eyre!(
                "Invalid chunk name with invalid component: {}",
                name
            ))
        }
    }
}

impl std::fmt::Display for ChunkName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "re_chunk_{:03}.pak", self.major_id)?;
        if let Some(patch_id) = self.patch_id {
            write!(f, ".patch_{:03}.pak", patch_id)?;
            return Ok(());
        }
        if let Some(sub_id) = self.sub_id {
            write!(f, ".sub_{:03}.pak", sub_id)?;
        }
        if let Some(sub_patch_id) = self.sub_patch_id {
            write!(f, ".patch_{:03}.pak", sub_patch_id)?;
        }

        Ok(())
    }
}

impl PartialOrd for ChunkName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major_id
            .cmp(&other.major_id)
            .then(self.sub_id.cmp(&other.sub_id))
            .then(self.patch_id.cmp(&other.patch_id))
            .then(self.sub_patch_id.cmp(&other.sub_patch_id))
    }
}

enum Component {
    Major(u32),
    Patch(u32),
    Sub(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_name_formats() {
        // Test base chunk
        let base = ChunkName::new(0);
        assert_eq!(base.to_string(), "re_chunk_000.pak");

        // Test patch chunk
        let patch = ChunkName::try_from_str("re_chunk_000.pak.patch_001.pak").unwrap();
        assert_eq!(patch.to_string(), "re_chunk_000.pak.patch_001.pak");

        // Test sub chunk
        let sub = ChunkName::try_from_str("re_chunk_000.pak.sub_000.pak").unwrap();
        assert_eq!(sub.to_string(), "re_chunk_000.pak.sub_000.pak");

        // Test sub patch chunk
        let sub_patch =
            ChunkName::try_from_str("re_chunk_000.pak.sub_000.pak.patch_001.pak").unwrap();
        assert_eq!(
            sub_patch.to_string(),
            "re_chunk_000.pak.sub_000.pak.patch_001.pak"
        );
    }
}

//! Chunk file name format
//!
//! File name structure:
//! - Base: re_chunk_XXX.pak
//! - Patch: re_chunk_XXX.pak.patch_XXX.pak
//! - Sub: re_chunk_XXX.pak.sub_XXX.pak
//! - Sub Patch: re_chunk_XXX.pak.sub_XXX.pak.patch_XXX.pak
//! - DLC: re_dlc_stm_3308900.pak (and more)

use color_eyre::eyre;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkComponent {
    /// Base chunk with major ID (re_chunk_XXX.pak)
    Base(u32),
    /// DLC chunk with DLC ID (re_dlc_stm_3308900.pak)
    Dlc(String),
    /// Patch chunk with patch ID (XXX in .patch_XXX.pak)
    Patch(u32),
    /// Sub chunk with sub ID (XXX in .sub_XXX.pak)
    Sub(u32),
    /// Sub patch chunk with sub patch ID (YYY in .sub_XXX.pak.patch_YYY.pak)
    SubPatch(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkName {
    /// Chunk components
    pub components: Vec<ChunkComponent>,
}

impl ChunkName {
    #[allow(dead_code)]
    /// Create a new base chunk name (re_chunk_XXX.pak)
    pub fn new(major_id: u32) -> Self {
        Self {
            components: vec![ChunkComponent::Base(major_id)],
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
        let part_pairs = dot_parts
            .chunks_exact(2)
            .map(|c| (c[0], c[1]))
            .collect::<Vec<(&str, &str)>>();

        // check if all parts have the correct extension
        if !part_pairs.iter().all(|(_, ext)| *ext == "pak") {
            return Err(eyre::eyre!(
                "Invalid chunk name with invalid extension: {}",
                name
            ));
        }

        let mut components = Vec::new();
        let mut has_sub = false;

        for (part_name, _) in part_pairs.iter() {
            let component = Self::parse_component(part_name)?;
            match component {
                Component::Major(id) => {
                    components.push(ChunkComponent::Base(id));
                }
                Component::Dlc(id) => {
                    components.push(ChunkComponent::Dlc(id));
                }
                Component::Sub(id) => {
                    components.push(ChunkComponent::Sub(id));
                    has_sub = true;
                }
                Component::Patch(id) => {
                    if has_sub {
                        components.push(ChunkComponent::SubPatch(id));
                    } else {
                        components.push(ChunkComponent::Patch(id));
                    }
                }
            }
        }

        Ok(Self { components })
    }

    /// Get the major ID (base chunk ID)
    pub fn major_id(&self) -> Option<u32> {
        self.components.iter().find_map(|c| match c {
            ChunkComponent::Base(id) => Some(*id),
            _ => None,
        })
    }

    /// Get the patch ID
    pub fn patch_id(&self) -> Option<u32> {
        self.components.iter().find_map(|c| match c {
            ChunkComponent::Patch(id) => Some(*id),
            _ => None,
        })
    }

    /// Get the sub ID
    pub fn sub_id(&self) -> Option<u32> {
        self.components.iter().find_map(|c| match c {
            ChunkComponent::Sub(id) => Some(*id),
            _ => None,
        })
    }

    /// Get the sub patch ID
    pub fn sub_patch_id(&self) -> Option<u32> {
        self.components.iter().find_map(|c| match c {
            ChunkComponent::SubPatch(id) => Some(*id),
            _ => None,
        })
    }

    /// Add a sub patch component with the given ID
    pub fn with_sub_patch(&self, patch_id: u32) -> Self {
        let mut new_components = self.components.clone();
        new_components.push(ChunkComponent::SubPatch(patch_id));
        Self {
            components: new_components,
        }
    }

    fn parse_component(name: &str) -> color_eyre::Result<Component> {
        if name.starts_with("re_chunk_") {
            let major_id = name
                .strip_prefix("re_chunk_")
                .unwrap()
                .parse::<u32>()
                .map_err(|e| eyre::eyre!("Chunk name with invalid major ID: {}", e))?;
            Ok(Component::Major(major_id))
        } else if name.starts_with("re_dlc_") {
            let dlc_id = name.strip_prefix("re_dlc_").unwrap().to_string();
            Ok(Component::Dlc(dlc_id))
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
        for (i, component) in self.components.iter().enumerate() {
            if i > 0 {
                write!(f, ".")?;
            }
            match component {
                ChunkComponent::Base(id) => write!(f, "re_chunk_{:03}.pak", id)?,
                ChunkComponent::Dlc(id) => write!(f, "re_dlc_{}.pak", id)?,
                ChunkComponent::Patch(id) => write!(f, "patch_{:03}.pak", id)?,
                ChunkComponent::Sub(id) => write!(f, "sub_{:03}.pak", id)?,
                ChunkComponent::SubPatch(id) => write!(f, "patch_{:03}.pak", id)?,
            }
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
        // compare by component count first
        self.components
            .len()
            .cmp(&other.components.len())
            .then_with(|| {
                // compare each component
                for (a, b) in self.components.iter().zip(other.components.iter()) {
                    let cmp = match (a, b) {
                        (ChunkComponent::Base(a), ChunkComponent::Base(b)) => a.cmp(b),
                        (ChunkComponent::Dlc(a), ChunkComponent::Dlc(b)) => a.cmp(b),
                        (ChunkComponent::Patch(a), ChunkComponent::Patch(b)) => a.cmp(b),
                        (ChunkComponent::Sub(a), ChunkComponent::Sub(b)) => a.cmp(b),
                        (ChunkComponent::SubPatch(a), ChunkComponent::SubPatch(b)) => a.cmp(b),
                        // compare by component type priority
                        (ChunkComponent::Base(_), _) => std::cmp::Ordering::Less,
                        (_, ChunkComponent::Base(_)) => std::cmp::Ordering::Greater,
                        (ChunkComponent::Dlc(_), _) => std::cmp::Ordering::Less,
                        (_, ChunkComponent::Dlc(_)) => std::cmp::Ordering::Greater,
                        (ChunkComponent::Sub(_), _) => std::cmp::Ordering::Less,
                        (_, ChunkComponent::Sub(_)) => std::cmp::Ordering::Greater,
                        (ChunkComponent::Patch(_), ChunkComponent::SubPatch(_)) => {
                            std::cmp::Ordering::Less
                        }
                        (ChunkComponent::SubPatch(_), ChunkComponent::Patch(_)) => {
                            std::cmp::Ordering::Greater
                        }
                    };
                    if cmp != std::cmp::Ordering::Equal {
                        return cmp;
                    }
                }
                std::cmp::Ordering::Equal
            })
    }
}

enum Component {
    Major(u32),
    Dlc(String),
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

        // Test DLC chunk
        let dlc = ChunkName::try_from_str("re_dlc_stm_3308900.pak").unwrap();
        assert_eq!(dlc.to_string(), "re_dlc_stm_3308900.pak");
    }

    #[test]
    fn test_chunk_helper_methods() {
        // Test base chunk helper methods
        let base = ChunkName::new(123);
        assert_eq!(base.major_id(), Some(123));
        assert_eq!(base.patch_id(), None);
        assert_eq!(base.sub_id(), None);
        assert_eq!(base.sub_patch_id(), None);

        // Test complex chunk helper methods
        let complex =
            ChunkName::try_from_str("re_chunk_456.pak.sub_789.pak.patch_012.pak").unwrap();
        assert_eq!(complex.major_id(), Some(456));
        assert_eq!(complex.patch_id(), None);
        assert_eq!(complex.sub_id(), Some(789));
        assert_eq!(complex.sub_patch_id(), Some(12));

        // Test DLC chunk helper methods
        let dlc = ChunkName::try_from_str("re_dlc_stm_3308900.pak").unwrap();
        assert_eq!(dlc.major_id(), None);
    }

    #[test]
    fn test_with_sub_patch() {
        let base = ChunkName::try_from_str("re_chunk_000.pak.sub_001.pak").unwrap();
        let with_patch = base.with_sub_patch(99);

        assert_eq!(with_patch.major_id(), Some(0));
        assert_eq!(with_patch.sub_id(), Some(1));
        assert_eq!(with_patch.sub_patch_id(), Some(99));
        assert_eq!(
            with_patch.to_string(),
            "re_chunk_000.pak.sub_001.pak.patch_099.pak"
        );
    }
}

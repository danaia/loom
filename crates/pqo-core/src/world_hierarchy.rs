//! GPU-oriented spatial hierarchy planning for large worlds.
//!
//! A hierarchy separates represented population from the clusters that need
//! simulation or rendering work in a frame. Plans are deterministic, compact,
//! and contain no host-side per-element traversal.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldHierarchyConfig {
    pub grid_width: u32,
    pub leaf_edge: u32,
    pub maximum_levels: u32,
    pub metadata_bytes_per_cluster: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldHierarchyPlan {
    pub represented_elements: u64,
    pub levels: Vec<WorldHierarchyLevel>,
    pub metadata_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldHierarchyLevel {
    pub level: u32,
    pub cluster_edge: u32,
    pub clusters_per_axis: u32,
    pub cluster_count: u64,
    pub maximum_elements_per_cluster: u64,
}

impl WorldHierarchyPlan {
    pub fn for_cubic_grid(config: WorldHierarchyConfig) -> Result<Self, String> {
        if config.grid_width == 0 || config.leaf_edge == 0 || config.maximum_levels == 0 {
            return Err("world hierarchy dimensions and level count must be non-zero".to_owned());
        }
        if !config.grid_width.is_multiple_of(config.leaf_edge) {
            return Err("world grid width must be divisible by the leaf-cluster edge".to_owned());
        }
        let represented_elements = u64::from(config.grid_width).pow(3);
        let mut levels = Vec::new();
        let mut edge = config.leaf_edge;
        for level in 0..config.maximum_levels {
            let clusters_per_axis = config.grid_width.div_ceil(edge);
            let cluster_count = u64::from(clusters_per_axis).pow(3);
            levels.push(WorldHierarchyLevel {
                level,
                cluster_edge: edge,
                clusters_per_axis,
                cluster_count,
                maximum_elements_per_cluster: u64::from(edge).pow(3),
            });
            if clusters_per_axis == 1 {
                break;
            }
            edge = edge.saturating_mul(2).min(config.grid_width);
        }
        let total_clusters = levels.iter().map(|level| level.cluster_count).sum::<u64>();
        Ok(Self {
            represented_elements,
            levels,
            metadata_bytes: total_clusters * u64::from(config.metadata_bytes_per_cluster),
        })
    }

    pub fn leaf_cluster_count(&self) -> u64 {
        self.levels.first().map_or(0, |level| level.cluster_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_grid_builds_compact_multilevel_hierarchy() {
        let plan = WorldHierarchyPlan::for_cubic_grid(WorldHierarchyConfig {
            grid_width: 100,
            leaf_edge: 4,
            maximum_levels: 6,
            metadata_bytes_per_cluster: 12,
        })
        .unwrap();
        assert_eq!(plan.represented_elements, 1_000_000);
        assert_eq!(plan.leaf_cluster_count(), 15_625);
        assert_eq!(
            plan.levels
                .iter()
                .map(|level| level.cluster_count)
                .collect::<Vec<_>>(),
            [15_625, 2_197, 343, 64, 8, 1]
        );
        assert!(plan.metadata_bytes < 256 * 1024);
    }

    #[test]
    fn billion_element_grid_has_bounded_hierarchy_metadata() {
        let plan = WorldHierarchyPlan::for_cubic_grid(WorldHierarchyConfig {
            grid_width: 1_000,
            leaf_edge: 8,
            maximum_levels: 8,
            metadata_bytes_per_cluster: 16,
        })
        .unwrap();
        assert_eq!(plan.represented_elements, 1_000_000_000);
        assert!(plan.metadata_bytes < 36 * 1024 * 1024);
    }
}

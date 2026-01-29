// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::path::Path;

use hnsw_rs::anndists::dist::distances::DistCosine;
use hnsw_rs::api::AnnT;
use hnsw_rs::hnsw::Hnsw;
use hnsw_rs::hnswio::HnswIo;
use srag_common::{Error, Result};

use super::store::Store;

const BASENAME: &str = "srag_vectors";
const MAX_NB_CONNECTION: usize = 16;
const MAX_LAYER: usize = 16;
const EF_CONSTRUCTION: usize = 200;
const DEFAULT_MAX_ELEMENTS: usize = 100_000;

/// wraps hnsw_rs for vector similarity search.
/// persistence is handled by dump/reload cycle.
/// we leak the HnswIo loader to get 'static lifetime for loaded indices.
pub struct VectorIndex {
    hnsw: Hnsw<'static, f32, DistCosine>,
    dimension: usize,
    next_id: usize,
    max_elements: usize,
    capacity_warned: bool,
    loaded_from_disk: bool,
}

impl VectorIndex {
    pub fn new(dimension: usize, max_elements: usize) -> Result<Self> {
        let hnsw = Hnsw::new(
            MAX_NB_CONNECTION,
            max_elements,
            MAX_LAYER,
            EF_CONSTRUCTION,
            DistCosine,
        );
        Ok(Self {
            hnsw,
            dimension,
            next_id: 0,
            max_elements,
            capacity_warned: false,
            loaded_from_disk: false,
        })
    }

    pub fn open(path: &Path, dimension: usize) -> Result<Self> {
        let graph_file = path.join(format!("{}.hnsw.graph", BASENAME));
        let data_file = path.join(format!("{}.hnsw.data", BASENAME));

        if graph_file.exists() && data_file.exists() {
            match Self::load_from_disk(path, dimension) {
                Ok(index) => {
                    tracing::info!("loaded hnsw index from disk ({} points)", index.len());
                    return Ok(index);
                }
                Err(e) => {
                    tracing::warn!("failed to load hnsw index: {}, will rebuild from db", e);
                }
            }
        }

        Self::new(dimension, DEFAULT_MAX_ELEMENTS)
    }

    fn load_from_disk(path: &Path, dimension: usize) -> Result<Self> {
        let loader = Box::new(HnswIo::new(path, BASENAME));
        let loader: &'static mut HnswIo = Box::leak(loader);

        let hnsw: Hnsw<'static, f32, DistCosine> = loader
            .load_hnsw()
            .map_err(|e| Error::Index(format!("Failed to load HNSW: {}", e)))?;

        let nb_point = hnsw.get_nb_point();

        Ok(Self {
            hnsw,
            dimension,
            next_id: nb_point,
            max_elements: DEFAULT_MAX_ELEMENTS.max(nb_point),
            capacity_warned: false,
            loaded_from_disk: true,
        })
    }

    pub fn insert(&mut self, id: usize, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dimension {
            return Err(Error::Index(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            )));
        }
        if !self.capacity_warned && self.len() >= self.max_elements {
            tracing::warn!(
                "hnsw index exceeded pre-allocated capacity of {} elements, performance may degrade",
                self.max_elements
            );
            self.capacity_warned = true;
        }
        self.hnsw.insert_slice((vector, id));
        if id >= self.next_id {
            self.next_id = id + 1;
        }
        Ok(())
    }

    pub fn search(&self, query: &[f32], k: usize, ef: usize) -> Result<Vec<(usize, f32)>> {
        if query.len() != self.dimension {
            return Err(Error::Index(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimension,
                query.len()
            )));
        }
        let results = self.hnsw.search(query, k, ef);
        Ok(results
            .into_iter()
            .map(|n| (n.get_origin_id(), n.get_distance()))
            .collect())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        self.hnsw
            .file_dump(path, BASENAME)
            .map_err(|e| Error::Index(format!("Failed to save HNSW index: {}", e)))?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.hnsw.get_nb_point()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn next_id(&self) -> usize {
        self.next_id
    }

    pub fn loaded_from_disk(&self) -> bool {
        self.loaded_from_disk
    }
}

pub fn rebuild_hnsw_from_db(store: &Store, index: &mut VectorIndex) -> Result<()> {
    if index.loaded_from_disk() || !index.is_empty() {
        return Ok(());
    }
    let total = store.embedding_count()?;
    if total == 0 {
        return Ok(());
    }
    tracing::info!("rebuilding hnsw index from {} embeddings", total);
    let dim = index.dimension();
    store.for_each_embedding(dim, |id, vector| index.insert(id as usize, &vector))?;
    tracing::info!("hnsw rebuild complete ({} points)", index.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const TEST_DIM: usize = 8;

    fn random_vector(dim: usize) -> Vec<f32> {
        (0..dim).map(|i| (i as f32 * 0.1) % 1.0).collect()
    }

    #[test]
    fn test_new_index() {
        let index = VectorIndex::new(TEST_DIM, 1000).unwrap();
        assert!(index.is_empty());
        assert_eq!(index.dimension(), TEST_DIM);
        assert_eq!(index.next_id(), 0);
    }

    #[test]
    fn test_insert_and_search() {
        let mut index = VectorIndex::new(TEST_DIM, 1000).unwrap();
        let v1 = random_vector(TEST_DIM);
        let v2: Vec<f32> = v1.iter().map(|x| x + 0.1).collect();

        index.insert(0, &v1).unwrap();
        index.insert(1, &v2).unwrap();

        assert_eq!(index.len(), 2);

        let results = index.search(&v1, 2, 100).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn test_dimension_mismatch_insert() {
        let mut index = VectorIndex::new(TEST_DIM, 1000).unwrap();
        let wrong_dim = vec![0.1; TEST_DIM + 1];
        let result = index.insert(0, &wrong_dim);
        assert!(result.is_err());
    }

    #[test]
    fn test_dimension_mismatch_search() {
        let index = VectorIndex::new(TEST_DIM, 1000).unwrap();
        let wrong_dim = vec![0.1; TEST_DIM + 1];
        let result = index.search(&wrong_dim, 5, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_search() {
        let index = VectorIndex::new(TEST_DIM, 1000).unwrap();
        let query = random_vector(TEST_DIM);
        let results = index.search(&query, 5, 100).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_save_and_open() {
        let dir = tempdir().unwrap();
        let dim = TEST_DIM;

        {
            let mut index = VectorIndex::new(dim, 1000).unwrap();
            let v1 = random_vector(dim);
            index.insert(0, &v1).unwrap();
            index.save(dir.path()).unwrap();
        }

        let loaded = VectorIndex::open(dir.path(), dim).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.loaded_from_disk());
    }

    #[test]
    fn test_open_nonexistent() {
        let dir = tempdir().unwrap();
        let index = VectorIndex::open(dir.path(), TEST_DIM).unwrap();
        assert!(index.is_empty());
        assert!(!index.loaded_from_disk());
    }

    #[test]
    fn test_next_id_tracking() {
        let mut index = VectorIndex::new(TEST_DIM, 1000).unwrap();
        let v = random_vector(TEST_DIM);

        index.insert(5, &v).unwrap();
        assert_eq!(index.next_id(), 6);

        index.insert(3, &v).unwrap();
        assert_eq!(index.next_id(), 6);

        index.insert(10, &v).unwrap();
        assert_eq!(index.next_id(), 11);
    }
}

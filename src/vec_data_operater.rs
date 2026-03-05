// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

use fountain_engine::algebra::finite_field::GF256;
use fountain_engine::types::Operation;
use fountain_engine::traits::DataOperator;
use std::collections::HashMap;

/// VecDataManager is a simple implementation of DataManager for a vector of data vectors.
/// It uses data vector IDs to access the data vectors for external use.
/// Internally, a vector of data vectors is used to store the data vectors.
/// The data vector IDs are used to map the data vectors to the data vector indices.
pub struct VecDataOperater {
    vectors: Vec<Vec<u8>>,
    vector_len: usize,
    //next_temp_vector_id: usize,
    gf256: GF256,
    /// Maps data vector ID to vector index
    data_id_to_index: HashMap<usize, usize>,
    // Maps vector index to data vector ID
    //index_to_id: HashMap<usize, usize>,
}

// const MAX_DATA_VECTOR_ID: usize = 10000000; //usize::MAX / 2;

impl VecDataOperater {
    /// Creates a new operator with the given vector length; initially holds no vectors.
    pub fn new(vector_len: usize) -> Self {
        Self {
            vectors: Vec::new(),
            vector_len,
            //next_temp_vector_id: MAX_DATA_VECTOR_ID+1,
            gf256: GF256::default(),
            data_id_to_index: HashMap::new(),
            //index_to_id: HashMap::new(),
        }
    }

    fn new_zero_vector(&self) -> Vec<u8> {
        vec![0u8; self.vector_len]
    }

    fn append_new_vector(&mut self, data_id: usize) -> usize {
        let new_index = self.vectors.len();
        self.data_id_to_index.insert(data_id, new_index);
        // dbg!("add data_id", data_id);
        //self.index_to_id.insert(new_index, vector_id);
        self.vectors.push(self.new_zero_vector());
        new_index
    }

    /// if a data vector exists, set it to zero
    /// if a data vector does not exist, create it and set it to zero
    /// return the index of the vector
    fn ensure_vector_exists_set_zero(&mut self, data_id: usize) -> usize {
        
        if let Some(idx) = self.data_id_to_index.get(&data_id) {
            self.vectors[*idx].iter_mut().for_each(|x| *x = 0);
            *idx
        } else {
            // If ID doesn't exist, create a new mapping
            self.append_new_vector(data_id)
        }
    }

    /// if a data vector does not exist, create it and set it to zero
    /// return the index of the vector
    fn ensure_vector_exists(&mut self, vector_id: usize) -> usize {
        
        if let Some(idx) = self.data_id_to_index.get(&vector_id) {
            *idx
        } else {
            self.append_new_vector(vector_id)
        }
    }

    fn remove_vector(&mut self, vector_id: usize) {
        if let Some(_index) = self.data_id_to_index.get(&vector_id) {
            // Removing a vector changes the index of the other vectors
            // so we do not remove the vector from the vector
            //self.vectors.remove(*index);
            self.data_id_to_index.remove(&vector_id);
            // dbg!("remove data_id", vector_id);
        } else {
            panic!("Vector with ID {} does not exist", vector_id);
        }
    }

    /// Solve the linear system Ax = b given the LU decomposition of A.
    /// This performs forward and backward substitution.
    fn _lu_solve(&mut self, a: &mut [Vec<u8>], target_ids: &[usize]) {
        if a.len() != target_ids.len() {
            panic!("The number of rows in A must be equal to the number of target IDs");
        }

        let n = a.len();

        // Forward substitution (L part)
        for j in 0..n - 1 {
            for i in j + 1..n {
                // self.vectors[i] = self.vectors[i] + self.vectors[j] * a[i][j]
                for k in 0..self.vector_len {
                    self.vectors[i][k] ^= self.gf256.multiply(a[i][j], self.vectors[j][k]);
                }
            }
        }

        // Backward substitution (U part)
        for j in (0..n).rev() {
            // Multiply by inverse of diagonal element
            let inv = self.gf256.inverse(a[j][j]);
            for k in 0..self.vector_len {
                self.vectors[j][k] = self.gf256.multiply(self.vectors[j][k], inv);
            }

            for i in (0..j).rev() {
                for k in 0..self.vector_len {
                    self.vectors[i][k] ^= self.gf256.multiply(a[i][j], self.vectors[j][k]);
                }
            }
        }
    }

    /// Sets a single byte at `vector_pos` in the vector identified by `vector_id`.
    pub fn set_vector(&mut self, vector_id: usize, vector_pos: usize, entry: u8) {
        if let Some(index) = self.data_id_to_index.get(&vector_id) {
            self.vectors[*index][vector_pos] = entry;
        } else {
            panic!("Vector with ID {} does not exist", vector_id);
        }
    }
}

impl DataOperator for VecDataOperater {
    /// Add a vector to the manager.
    fn insert_vector(&mut self, src: &[u8], data_id: usize) {
        let target_index = self.ensure_vector_exists(data_id);
        if src.len() != self.vector_len {
            panic!(
                "The length of the vector to add must be equal to the vector length: {} != {}",
                src.len(),
                self.vector_len
            );
        }
        self.vectors[target_index] = src.to_vec();
    }

    fn get_vector(&self, data_id: usize) -> &[u8] {
        if let Some(index) = self.data_id_to_index.get(&data_id) {
            &self.vectors[*index]
        } else {
            panic!("Vector with ID {} does not exist", data_id);
        }
    }

    fn execute(&mut self, operation: &Operation) {
        match operation {
            Operation::EnsureZero { list_id } => {
                self.ensure_zero(list_id);
            }
            Operation::MultiplyAlpha { id } => {
                self.multiply_alpha(*id);
            }
            Operation::MultiplyScalar { scalar, id } => {
                self.multiply_scalar(*scalar, *id);
            }
            //Operation::DivideScalar { scalar, id } => {
            //    self.divide_scalar(*scalar, *id);
            //}
            Operation::AddToVector { list_id, target_id } => {
                self.add_to_vector(list_id, *target_id);
            }
            Operation::BroadcastAdd { src_id, target_ids } => {
                self.broadcast_add(*src_id, target_ids);
            }
            Operation::MulAdd {
                src_id,
                scalar,
                target_id,
            } => {
                self.mul_add(*src_id, *scalar, *target_id);
            }
            Operation::MoveTo { src_id, target_id } => {
                self.move_to(*src_id, *target_id);
            }
            Operation::CopyTo { src_id, target_id } => {
                self.copy_to(*src_id, *target_id);
            }
            Operation::Remove { id } => {
                self.remove(*id);
            }
            Operation::InfoCodedVector { .. } => {
            }
        }
    }
}

impl VecDataOperater {
    // fn permute_vectors(&mut self, ids: &[usize], perm: &[usize]) {
    //     if ids.len() != perm.len() {
    //         panic!("The number of IDs must be equal to the number of permutations");
    //     }
    //
    //     let end = ids.len();
    //     let index1 = self.id_to_index.get(&ids[end-1]).unwrap();
    //     let index2 = self.id_to_index.get(&ids[perm[end-1]]).unwrap();
    //     self.vectors.swap(*index1, *index2);
    //     // replace the element in perm with value end-1 to value perm[end-1]
    //
    //     if ids.len() == 2 {
    //         return;
    //     } else {
    //         let mut perm2 = perm[..end-1].to_vec();
    //         perm2.iter_mut().for_each(|x| if *x == end-1 { *x = perm[end-1] });
    //         self.permute_vectors(&ids[..end-1], &perm2);
    //     }
    // }

    /// initialize the vectors in the list to zero
    fn ensure_zero(&mut self, list_id: &[usize]) {
        for &id in list_id {
            self.ensure_vector_exists_set_zero(id);
        }
    }

    fn multiply_alpha(&mut self, vector_id: usize) {
        if let Some(index) = self.data_id_to_index.get(&vector_id) {
            let vector = &mut self.vectors[*index];
            for byte in vector.iter_mut() {
                *byte = self.gf256.mul_alpha(*byte);
            }
        } else {
            panic!("Vector with ID {} does not exist", vector_id);
        }
    }

    /* divide_scalar is not supported by the VecDataOperater
    fn divide_scalar(&mut self, scalar: u8, vector_id: usize) {
        if let Some(index) = self.data_id_to_index.get(&vector_id) {
            if scalar == 1 {
                return;
            }
            let inv = self.gf256.inverse(scalar);
            let vector = &mut self.vectors[*index];
            for byte in vector.iter_mut() {
                *byte = self.gf256.mul_lookup(*byte, inv);
            }
        } else {
            panic!("Vector with ID {} does not exist", vector_id);
        }
    } */

    fn multiply_scalar(&mut self, scalar: u8, vector_id: usize) {
        if let Some(index) = self.data_id_to_index.get(&vector_id) {
            // Implement proper scalar multiplication in GF(256)
            if scalar == 0 {
                // Multiplication by 0 results in zero packet
                self.vectors[*index].iter_mut().for_each(|x| *x = 0);
            } else if scalar == 1 {
                // Multiplication by 1 leaves packet unchanged
                // No operation needed
                //self.ensure_vector_exists(*index);
            } else {
                // Proper GF(256) multiplication
                let vector = &mut self.vectors[*index];
                for byte in vector.iter_mut() {
                    *byte = self.gf256.mul_lookup(*byte, scalar);
                }
            }
        } else {
            panic!("Vector with ID {} does not exist", vector_id);
        }
    }

    fn add_to_vector(&mut self, list_id: &[usize], target_id: usize) {
        // Ensure the target vector exists and get its index
        if let Some(target_index) = self.data_id_to_index.get(&target_id) {
            // Add each source vector to the target vector
            for &id in list_id {
                if let Some(&src_index) = self.data_id_to_index.get(&id) {
                    for i in 0..self.vector_len {
                        self.vectors[*target_index][i] ^= self.vectors[src_index][i];
                    }
                } else {
                    //panic!("Source vector with ID {} does not exist", id);
                    // skip non-exist data vectors
                }
            }
        } else {
            panic!("Target vector with ID {} does not exist", target_id);
        }
    }

    fn broadcast_add(&mut self, source_id: usize, target_ids: &[usize]) {
        if let Some(&src_index) = self.data_id_to_index.get(&source_id) {
            // Add the source vector to each target vector
            for &target_id in target_ids {
                if let Some(target_index) = self.data_id_to_index.get(&target_id) {
                    for i in 0..self.vector_len {
                        self.vectors[*target_index][i] ^= self.vectors[src_index][i];
                    }
                } else {
                    panic!("Target vector with ID {} does not exist", target_id);
                }
            }
        } else {
            panic!("Source vector with ID {} does not exist", source_id);
        }
    }

    fn mul_add(&mut self, source_id: usize, scalar: u8, target_id: usize) {
        if let Some(target_index) = self.data_id_to_index.get(&target_id) {
            if let Some(src_index) = self.data_id_to_index.get(&source_id) {
                match scalar {
                    0 => {
                        self.vectors[*target_index].iter_mut().for_each(|x| *x = 0);
                    }
                    1 => {
                        self.add_to_vector(&[source_id], target_id);
                    }
                    _ => {
                        for i in 0..self.vector_len {
                            self.vectors[*target_index][i] ^=
                                self.gf256.mul_lookup(scalar, self.vectors[*src_index][i]);
                        }
                    }
                }
            } else {
                panic!("Vector with ID {} does not exist", source_id);
            }
        } else {
            panic!("Vector with ID {} does not exist", target_id);
        }
    }

    fn move_to(&mut self, src_id: usize, target_id: usize) {
        if src_id == target_id {
            return;
        }
        if let Some(&src_index) = self.data_id_to_index.get(&src_id) {
            //let target_index = self.ensure_vector_exists(target_id);
            //self.vectors.swap(src_index, target_index);
            // todo: remove the src_id from the id_to_index and the data vector
            self.data_id_to_index.remove(&src_id);
            self.data_id_to_index.insert(target_id, src_index);
            // dbg!("move data_id", src_id, target_id);
        } else {
            panic!("Vector with ID {} does not exist", src_id);
        }
    }

    fn copy_to(&mut self, src_id: usize, target_id: usize) {
        if src_id == target_id {
            return;
        }
        if let Some(&src_index) = self.data_id_to_index.get(&src_id) {
            let target_index = self.ensure_vector_exists(target_id);
            self.vectors[target_index] = self.vectors[src_index].clone();
        } else {
            panic!("Source vector with ID {} does not exist", src_id);
        }
    }

    fn remove(&mut self, id: usize) {
        self.remove_vector(id);
    }
}

/// Test the VecPkgManager
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permute_vectors() {
        let mut manager = VecDataOperater::new(3);
        manager.insert_vector(&[1, 2, 3], 0);
        manager.insert_vector(&[4, 5, 6], 1);
        manager.insert_vector(&[7, 8, 9], 2);
        manager.insert_vector(&[10, 11, 12], 3);
        //manager.permute_vectors(&[0, 1, 2, 3], &[0, 1, 2, 3]);
        assert_eq!(manager.get_vector(1), vec![4, 5, 6]);
        assert_eq!(manager.get_vector(2), vec![7, 8, 9]);
        assert_eq!(manager.get_vector(3), vec![10, 11, 12]);
        assert_eq!(manager.get_vector(0), vec![1, 2, 3]);
    }
}

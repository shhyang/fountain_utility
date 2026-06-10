// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

//! Generic padding-aware encoder and decoder wrappers.
//!
//! Delegates to [`Encoder::new_with_num_source`](fountain_engine::Encoder::new_with_num_source)
//! and [`Decoder::new_with_num_source`](fountain_engine::Decoder::new_with_num_source).
//! Padding policy (systematic / ordinary) is installed inside the engine.

use fountain_engine::traits::{CodeScheme, DataOperator, PrecodePair};
use fountain_engine::types::{CodeParams, CodeType, DecodeStatus, DecodingConfig, DegreeSetFn};
use fountain_engine::{Decoder, Encoder};
use std::marker::PhantomData;

/// Block-size metadata for schemes with `source_k ≤ block_k`.
pub trait BlockSizePolicy: CodeScheme {
    /// Application source block size (payload symbol count).
    fn source_symbols(&self) -> usize {
        self.get_params().k
    }

    /// Internal block size used by the engine (`CodeParams.k`).
    fn block_symbols(&self) -> usize {
        self.get_params().k
    }

    /// Number of implicit zero padding symbols (`block_k − source_k`).
    fn num_padding(&self) -> usize {
        self.block_symbols()
            .saturating_sub(self.source_symbols())
    }
}

/// View a scheme at internal block size `CodeParams.k` with a smaller source count.
#[derive(Clone, Debug)]
pub struct WithSourceK<S> {
    /// Underlying scheme (parameterized at block size K′).
    pub scheme: S,
    /// Application source block size K.
    pub source_k: usize,
}

impl<S: CodeScheme> WithSourceK<S> {
    #[must_use]
    pub fn new(scheme: S, source_k: usize) -> Self {
        let block_k = scheme.get_params().k;
        assert!(
            source_k <= block_k,
            "source_k ({source_k}) must be <= block_k ({block_k})"
        );
        Self { scheme, source_k }
    }
}

impl<S: CodeScheme> CodeScheme for WithSourceK<S> {
    fn get_params(&self) -> CodeParams {
        self.scheme.get_params()
    }

    fn code_type(&self) -> CodeType {
        self.scheme.code_type()
    }

    fn create_degree_set_fn(&self) -> DegreeSetFn {
        self.scheme.create_degree_set_fn()
    }

    fn create_precode(&self) -> PrecodePair {
        self.scheme.create_precode()
    }

    fn decoding_config(&self) -> DecodingConfig {
        self.scheme.decoding_config()
    }
}

impl<S: CodeScheme> BlockSizePolicy for WithSourceK<S> {
    fn source_symbols(&self) -> usize {
        self.source_k
    }

    fn block_symbols(&self) -> usize {
        self.scheme.get_params().k
    }
}

fn infer_symbol_len<S: BlockSizePolicy>(
    scheme: &S,
    operator: &dyn DataOperator,
    symbol_len: usize,
) -> usize {
    if symbol_len > 0 {
        return symbol_len;
    }
    let source_k = scheme.source_symbols();
    if source_k > 0 {
        let len = operator.get_vector(0).len();
        assert!(
            len > 0,
            "symbol_len is 0; pass symbol_len or insert source symbol 0 first"
        );
        return len;
    }
    panic!("symbol_len must be provided when source_k = 0");
}

/// Zero-fill systematic padding message slots in an operator before encoding with real bytes.
fn install_systematic_operator_padding<S: BlockSizePolicy>(
    scheme: &S,
    operator: &mut dyn DataOperator,
    symbol_len: usize,
) {
    if scheme.code_type() != CodeType::Systematic || scheme.num_padding() == 0 {
        return;
    }
    let len = infer_symbol_len(scheme, operator, symbol_len);
    let zeros = vec![0u8; len];
    for id in scheme.source_symbols()..scheme.block_symbols() {
        operator.insert_vector(&zeros, id);
    }
}

/// Encoder wrapper that configures `num_source` on the engine before precoding.
pub struct PaddedEncoder<S: BlockSizePolicy + Clone> {
    /// Underlying fountain engine encoder (`CodeParams.k` = block size).
    pub inner: Encoder,
    source_k: usize,
    block_k: usize,
    _marker: PhantomData<S>,
}

/// Decoder wrapper that configures `num_source` on the engine before solver init.
pub struct PaddedDecoder<S: BlockSizePolicy + Clone> {
    /// Underlying fountain engine decoder.
    pub inner: Decoder,
    source_k: usize,
    block_k: usize,
    _marker: PhantomData<S>,
}

impl<S: BlockSizePolicy + Clone> PaddedEncoder<S> {
    /// Creates an encoder without an application data operator.
    pub fn new(scheme: S) -> Self {
        let source_k = scheme.source_symbols();
        let block_k = scheme.block_symbols();
        let inner = Encoder::new_with_num_source(&scheme, source_k);
        Self {
            inner,
            source_k,
            block_k,
            _marker: PhantomData,
        }
    }

    /// Creates an encoder with a data operator. Systematic padding bytes are zero-filled
    /// in the operator before precoding; ordinary padding is handled by the engine.
    pub fn new_with_operator(
        scheme: S,
        mut operator: Box<dyn DataOperator>,
        symbol_len: usize,
    ) -> Self {
        let source_k = scheme.source_symbols();
        let block_k = scheme.block_symbols();
        install_systematic_operator_padding(&scheme, &mut *operator, symbol_len);
        let inner = Encoder::new_with_operator_and_num_source(&scheme, operator, source_k);
        Self {
            inner,
            source_k,
            block_k,
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn source_symbols(&self) -> usize {
        self.source_k
    }

    #[must_use]
    pub fn block_symbols(&self) -> usize {
        self.block_k
    }

    #[must_use]
    pub fn num_padding(&self) -> usize {
        self.block_k.saturating_sub(self.source_k)
    }

    #[must_use]
    pub fn first_padding_symbol(&self) -> usize {
        self.source_k
    }

    #[must_use]
    pub fn is_source_symbol(&self, coded_id: usize) -> bool {
        coded_id < self.source_k
    }

    #[must_use]
    pub fn is_padding_symbol(&self, coded_id: usize) -> bool {
        (self.source_k..self.block_k).contains(&coded_id)
    }

    pub fn encode_coded_vector(&mut self, coded_id: usize) -> Option<usize> {
        self.inner.encode_coded_vector(coded_id)
    }

    pub fn get_data_vector(&self, data_id: usize) -> &[u8] {
        self.inner.get_data_vector(data_id)
    }
}

impl<S: BlockSizePolicy + Clone> PaddedDecoder<S> {
    /// Creates a decoder with implicit padding installed during solver initialization.
    pub fn new(scheme: S) -> Self {
        let source_k = scheme.source_symbols();
        let block_k = scheme.block_symbols();
        let inner = Decoder::new_with_num_source(&scheme, source_k);
        Self {
            inner,
            source_k,
            block_k,
            _marker: PhantomData,
        }
    }

    /// Creates a decoder with a data operator attached.
    pub fn new_with_operator(
        scheme: S,
        operator: Box<dyn DataOperator>,
        _symbol_len: usize,
    ) -> Self {
        Self::new_with_operator_impl(scheme, operator, false)
    }

    /// Like [`Self::new_with_operator`], but skips recording decode operations (execute-only).
    pub fn new_with_operator_execute_only(
        scheme: S,
        operator: Box<dyn DataOperator>,
        _symbol_len: usize,
    ) -> Self {
        Self::new_with_operator_impl(scheme, operator, true)
    }

    fn new_with_operator_impl(
        scheme: S,
        operator: Box<dyn DataOperator>,
        execute_only: bool,
    ) -> Self {
        let source_k = scheme.source_symbols();
        let block_k = scheme.block_symbols();
        let inner = if execute_only {
            Decoder::new_with_operator_execute_only_and_num_source(&scheme, operator, source_k)
        } else {
            Decoder::new_with_operator_and_num_source(&scheme, operator, source_k)
        };
        Self {
            inner,
            source_k,
            block_k,
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn source_symbols(&self) -> usize {
        self.source_k
    }

    #[must_use]
    pub fn block_symbols(&self) -> usize {
        self.block_k
    }

    #[must_use]
    pub fn num_padding(&self) -> usize {
        self.block_k.saturating_sub(self.source_k)
    }

    pub fn decode_status(&self) -> DecodeStatus {
        self.inner.decode_status()
    }

    pub fn add_coded_vector(&mut self, coded_id: usize, vector: &[u8]) -> DecodeStatus {
        self.inner.add_coded_vector(coded_id, vector)
    }

    pub fn add_coded_id(&mut self, coded_id: usize) -> DecodeStatus {
        self.inner.add_coded_id(coded_id)
    }

    pub fn get_data_vector(&self, data_id: usize) -> &[u8] {
        self.inner.manager.get_data_vector(data_id)
    }
}

/// [`RealSymbolSession`](crate::RealSymbolSession) that applies [`BlockSizePolicy`] padding.
#[derive(Debug, Clone, Copy)]
pub struct PaddedRealSymbolSession;

impl<S: BlockSizePolicy + Clone> crate::RealSymbolSession<S> for PaddedRealSymbolSession {
    fn prepare_encode_operator(
        &self,
        scheme: &S,
        operator: &mut dyn DataOperator,
        messages: &[Vec<u8>],
    ) {
        for (i, vector) in messages.iter().enumerate() {
            operator.insert_vector(vector, i);
        }
        let symbol_len = messages.first().map_or(0, Vec::len);
        install_systematic_operator_padding(scheme, operator, symbol_len);
    }

    fn new_delayed_encoder(&self, scheme: S) -> Encoder {
        PaddedEncoder::new(scheme).inner
    }

    fn new_delayed_decoder(&self, scheme: &S) -> Decoder {
        PaddedDecoder::new(scheme.clone()).inner
    }

    fn coded_payload(&self, encoder: &Encoder, coded_id: usize) -> Vec<u8> {
        encoder.manager.get_coded_vector(coded_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct DummyScheme {
        k: usize,
        a: usize,
        code_type: CodeType,
    }

    impl CodeScheme for DummyScheme {
        fn get_params(&self) -> CodeParams {
            CodeParams::new(self.k, self.a, 0, 0)
        }

        fn code_type(&self) -> CodeType {
            self.code_type
        }

        fn create_degree_set_fn(&self) -> DegreeSetFn {
            let k = self.k;
            Box::new(move |coded_id| {
                if coded_id >= k {
                    vec![0]
                } else {
                    (0..k).collect()
                }
            })
        }

        fn create_precode(&self) -> PrecodePair {
            (None, None)
        }
    }

    impl BlockSizePolicy for DummyScheme {
        fn source_symbols(&self) -> usize {
            self.k - 2
        }
    }

    #[test]
    fn padded_encoder_delegates_num_source_to_engine() {
        let scheme = DummyScheme {
            k: 12,
            a: 12,
            code_type: CodeType::Systematic,
        };
        let source_k = scheme.source_symbols();
        let enc = Encoder::new_without_precoding_with_num_source(&scheme, source_k);
        assert_eq!(enc.manager.num_source(), source_k);
        assert_eq!(enc.manager.num_padding(), 2);
    }

    #[test]
    fn padded_decoder_registers_systematic_padding() {
        let scheme = DummyScheme {
            k: 12,
            a: 12,
            code_type: CodeType::Systematic,
        };
        let dec = PaddedDecoder::new(scheme);
        assert_eq!(dec.inner.manager.num_source(), 10);
        assert!(dec.inner.manager.data_id_of_coded_vector(10).is_some());
        assert!(dec.inner.manager.data_id_of_coded_vector(11).is_some());
    }

    #[test]
    fn with_source_k_enforces_block_size() {
        let inner = DummyScheme {
            k: 12,
            a: 11,
            code_type: CodeType::Ordinary,
        };
        let wrapped = WithSourceK::new(inner, 10);
        assert_eq!(wrapped.source_symbols(), 10);
        assert_eq!(wrapped.block_symbols(), 12);
        assert_eq!(wrapped.num_padding(), 2);
    }
}

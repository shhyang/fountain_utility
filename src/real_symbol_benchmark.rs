// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

//! Real-symbol encode/decode benchmarks with on-the-fly and deferred operator execution.
//!
//! See [doc-engine.org](../../docs/doc-engine.org) § Delayed Data Operation for the
//! deferred execution model: the engine records [`Operation`]s first; the caller replays
//! them on a [`DataOperator`] in a separate phase.

use std::collections::HashMap;
use std::time::Instant;

use fountain_engine::traits::{CodeScheme, DataOperator};
use fountain_engine::types::{CodeParams, DecodeStatus, Operation};
use fountain_engine::{Decoder, Encoder};

/// Builds a fresh in-memory operator for the given symbol length.
pub type OperatorFactory = fn(usize) -> Box<dyn DataOperator>;

/// Inputs shared by on-the-fly and deferred real-symbol benchmarks.
#[derive(Debug, Clone)]
pub struct RealSymbolBenchConfig {
    pub source_k: usize,
    pub symbol_size: usize,
    pub coded_ids: Vec<usize>,
    pub field_pp: u16,
}

impl RealSymbolBenchConfig {
    #[must_use]
    pub fn new<C: CodeScheme>(scheme: &C, source_k: usize, symbol_size: usize, num_coded: usize) -> Self {
        Self {
            source_k,
            symbol_size,
            coded_ids: systematic_coded_ids(scheme, source_k, num_coded),
            field_pp: fountain_engine::types::GF2_FIELD_POLY,
        }
    }

    #[must_use]
    pub fn with_field_pp(mut self, field_pp: u16) -> Self {
        self.field_pp = field_pp;
        self
    }
}

/// Scheme-specific hooks for codecs that wrap [`Encoder`] / [`Decoder`] (e.g. RFC 6330 padding).
pub trait RealSymbolSession<C: CodeScheme + Clone> {
    fn prepare_encode_operator(&self, scheme: &C, operator: &mut dyn DataOperator, messages: &[Vec<u8>]);

    fn new_delayed_encoder(&self, scheme: C) -> Encoder;

    fn new_delayed_decoder(&self, scheme: &C) -> Decoder;

    fn coded_payload(&self, encoder: &Encoder, coded_id: usize) -> Vec<u8> {
        encoder.manager.get_coded_vector(coded_id)
    }
}

/// Default session for schemes that use plain [`Encoder`] / [`Decoder`].
#[derive(Debug, Clone, Copy)]
pub struct StandardRealSymbolSession;

impl<C: CodeScheme + Clone> RealSymbolSession<C> for StandardRealSymbolSession {
    fn prepare_encode_operator(
        &self,
        _scheme: &C,
        operator: &mut dyn DataOperator,
        messages: &[Vec<u8>],
    ) {
        for (i, vector) in messages.iter().enumerate() {
            operator.insert_vector(vector, i);
        }
    }

    fn new_delayed_encoder(&self, scheme: C) -> Encoder {
        Encoder::new(&scheme)
    }

    fn new_delayed_decoder(&self, scheme: &C) -> Decoder {
        Decoder::new(scheme)
    }
}

/// Deterministic message payloads (`code_testing`-style).
#[must_use]
pub fn make_test_messages(source_k: usize, symbol_size: usize) -> Vec<Vec<u8>> {
    let mut message_vectors = vec![vec![0u8; symbol_size]; source_k];
    for (i, row) in message_vectors.iter_mut().enumerate() {
        for (j, byte) in row.iter_mut().enumerate() {
            *byte = ((i * 7 + j * 13) % 256) as u8;
        }
    }
    message_vectors
}

/// Systematic coded IDs: sources `0..source_k-1` plus repair symbols after `num_total`.
#[must_use]
pub fn systematic_coded_ids<C: CodeScheme>(
    scheme: &C,
    source_k: usize,
    num_coded: usize,
) -> Vec<usize> {
    let params = scheme.get_params();
    systematic_coded_ids_from_params(&params, source_k, num_coded)
}

#[must_use]
pub fn systematic_coded_ids_from_params(
    params: &CodeParams,
    source_k: usize,
    num_coded: usize,
) -> Vec<usize> {
    let mut ids: Vec<usize> = (0..source_k).collect();
    ids.extend(
        params.num_total()..params.num_total() + num_coded.saturating_sub(source_k),
    );
    ids
}

/// Wall-clock timings for on-the-fly operator execution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OnTheFlyTimings {
    /// Precoding phase: [`Encoder::new_with_operator_execute_only`] (engine + inline operator).
    pub precoding_ms: f64,
    /// LT encode loop with inline operator execute.
    pub lt_ms: f64,
    pub decode_ms: f64,
    /// Packets processed until [`DecodeStatus::Decoded`].
    pub decode_packets: usize,
}

impl OnTheFlyTimings {
    #[must_use]
    pub fn total_encode_ms(&self) -> f64 {
        self.precoding_ms + self.lt_ms
    }
}

/// Wall-clock timings for deferred execution (`engine + operator` per phase).
///
/// Encode is split into precoding and LT encoding, each with separate engine and operator
/// replay times (see `doc-engine.org` § Delayed Data Operation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeferredTimings {
    pub engine_precoding_ms: f64,
    pub operator_precoding_ms: f64,
    pub engine_encoding_ms: f64,
    pub operator_encoding_ms: f64,
    pub engine_decode_ms: f64,
    pub operator_decode_ms: f64,
}

impl DeferredTimings {
    #[must_use]
    pub fn total_encode_ms(&self) -> f64 {
        self.engine_precoding_ms
            + self.operator_precoding_ms
            + self.engine_encoding_ms
            + self.operator_encoding_ms
    }

    #[must_use]
    pub fn total_decode_ms(&self) -> f64 {
        self.engine_decode_ms + self.operator_decode_ms
    }
}

/// Replay operations on an operator (see `doc-engine.org` § Delayed Data Operation).
pub fn replay_operations(operator: &mut dyn DataOperator, ops: &[Operation]) {
    for op in ops {
        operator.execute(op);
    }
}

fn count_message_mismatches(
    operator: &dyn DataOperator,
    messages: &[Vec<u8>],
    decoded: bool,
) -> usize {
    if !decoded {
        return messages.len();
    }
    messages
        .iter()
        .enumerate()
        .filter(|(i, expected)| operator.get_vector(*i) != expected.as_slice())
        .count()
}

/// On-the-fly encode/decode with an attached operator (production-style interleaving).
pub fn benchmark_on_the_fly<C, S>(
    scheme: &C,
    session: &S,
    config: &RealSymbolBenchConfig,
    messages: &[Vec<u8>],
    make_operator: OperatorFactory,
) -> Result<OnTheFlyTimings, String>
where
    C: CodeScheme + Clone,
    S: RealSymbolSession<C>,
{
    assert_eq!(messages.len(), config.source_k);

    let mut enc_op = make_operator(config.symbol_size);
    enc_op.config_finite_field(config.field_pp);
    session.prepare_encode_operator(scheme, enc_op.as_mut(), messages);

    let t_pre = Instant::now();
    let mut encoder = Encoder::new_with_operator_execute_only(scheme, enc_op);
    let precoding_ms = t_pre.elapsed().as_secs_f64() * 1000.0;

    let t_lt = Instant::now();
    let mut payloads: HashMap<usize, Vec<u8>> =
        HashMap::with_capacity(config.coded_ids.len());
    for &coded_id in &config.coded_ids {
        if encoder.encode_coded_vector(coded_id).is_some() {
            payloads.insert(coded_id, session.coded_payload(&encoder, coded_id));
        }
    }
    let lt_ms = t_lt.elapsed().as_secs_f64() * 1000.0;

    let mut dec_op = make_operator(config.symbol_size);
    dec_op.config_finite_field(config.field_pp);
    let mut decoder = Decoder::new_with_operator_execute_only(scheme, dec_op);

    let t_dec = Instant::now();
    let mut decoded = false;
    let mut decode_packets = 0usize;
    for &coded_id in &config.coded_ids {
        if let Some(payload) = payloads.get(&coded_id) {
            decode_packets += 1;
            if decoder.add_coded_vector(coded_id, payload) == DecodeStatus::Decoded {
                decoded = true;
                break;
            }
        }
    }
    let decode_ms = t_dec.elapsed().as_secs_f64() * 1000.0;

    if !decoded {
        return Err("decode did not reach Decoded status".into());
    }

    let dec_op = decoder.manager.move_operator();
    let mismatches = count_message_mismatches(&*dec_op, messages, decoded);
    if mismatches != 0 {
        return Err(format!("{mismatches} message mismatch(es)"));
    }

    Ok(OnTheFlyTimings {
        precoding_ms,
        lt_ms,
        decode_ms,
        decode_packets,
    })
}

/// Deferred execution: engine records ops, then operator replays them in batch.
pub fn benchmark_deferred<C, S>(
    scheme: &C,
    session: &S,
    config: &RealSymbolBenchConfig,
    messages: &[Vec<u8>],
    make_operator: OperatorFactory,
) -> Result<DeferredTimings, String>
where
    C: CodeScheme + Clone,
    S: RealSymbolSession<C>,
{
    assert_eq!(messages.len(), config.source_k);

    let t_engine_precoding = Instant::now();
    let mut encoder = session.new_delayed_encoder(scheme.clone());
    let precoding_ops = encoder.manager.move_new_operations();
    let engine_precoding_ms = t_engine_precoding.elapsed().as_secs_f64() * 1000.0;

    let t_engine_encoding = Instant::now();
    let mut encode_mappings = HashMap::new();
    for &coded_id in &config.coded_ids {
        if let Some(data_id) = encoder.encode_coded_vector(coded_id) {
            encode_mappings.insert(coded_id, data_id);
        }
    }
    let encoding_ops = encoder.manager.move_new_operations();
    let engine_encoding_ms = t_engine_encoding.elapsed().as_secs_f64() * 1000.0;

    let t_operator_precoding = Instant::now();
    let mut encode_operator = make_operator(config.symbol_size);
    encode_operator.config_finite_field(config.field_pp);
    session.prepare_encode_operator(scheme, encode_operator.as_mut(), messages);
    replay_operations(encode_operator.as_mut(), &precoding_ops);
    let operator_precoding_ms = t_operator_precoding.elapsed().as_secs_f64() * 1000.0;

    let t_operator_encoding = Instant::now();
    replay_operations(encode_operator.as_mut(), &encoding_ops);
    let operator_encoding_ms = t_operator_encoding.elapsed().as_secs_f64() * 1000.0;

    let mut payloads: HashMap<usize, Vec<u8>> =
        HashMap::with_capacity(encode_mappings.len());
    for (&coded_id, &data_id) in &encode_mappings {
        payloads.insert(
            coded_id,
            encode_operator.get_vector(data_id).to_vec(),
        );
    }

    let t_engine_dec = Instant::now();
    let mut decoder = session.new_delayed_decoder(scheme);
    let init_ops = decoder.manager.move_new_operations();
    let mut decoded = false;
    for &coded_id in &config.coded_ids {
        if decoder.add_coded_id(coded_id) == DecodeStatus::Decoded {
            decoded = true;
            break;
        }
    }
    let decoding_ops = decoder.manager.move_new_operations();
    let decode_mappings = coded_id_mappings_from_ops(&init_ops, &decoding_ops);
    let engine_decode_ms = t_engine_dec.elapsed().as_secs_f64() * 1000.0;

    let t_operator_dec = Instant::now();
    let mut decode_operator = make_operator(config.symbol_size);
    decode_operator.config_finite_field(config.field_pp);
    session.prepare_encode_operator(scheme, decode_operator.as_mut(), messages);
    for &coded_id in &config.coded_ids {
        if let (Some(payload), Some(&data_id)) =
            (payloads.get(&coded_id), decode_mappings.get(&coded_id))
        {
            decode_operator.insert_vector(payload, data_id);
        }
    }
    replay_operations(decode_operator.as_mut(), &init_ops);
    replay_operations(decode_operator.as_mut(), &decoding_ops);
    let operator_decode_ms = t_operator_dec.elapsed().as_secs_f64() * 1000.0;

    let mismatches = count_message_mismatches(&*decode_operator, messages, decoded);
    if !decoded {
        return Err("decode did not reach Decoded status".into());
    }
    if mismatches != 0 {
        return Err(format!("{mismatches} message mismatch(es)"));
    }

    Ok(DeferredTimings {
        engine_precoding_ms,
        operator_precoding_ms,
        engine_encoding_ms,
        operator_encoding_ms,
        engine_decode_ms,
        operator_decode_ms,
    })
}

pub fn coded_id_mappings_from_ops(
    init_ops: &[Operation],
    decode_ops: &[Operation],
) -> HashMap<usize, usize> {
    let mut mappings = HashMap::new();
    for op in init_ops.iter().chain(decode_ops) {
        if let Operation::InfoCodedVector { coded_id, data_id } = op {
            mappings.insert(*coded_id, *data_id);
        }
    }
    mappings
}

/// Footnotes for [`print_real_symbol_benchmark_table`] (plan C1 in
/// `docs/plans/deferred-benchmark-optimization.md`).
pub fn print_real_symbol_benchmark_footnotes() {
    println!();
    println!("Notes (all times in ms):");
    println!(
        "  on-the-fly enc — full encode (otf_pre + otf_lt); otf_pre = precoding ctor, \
         otf_lt = LT loop; both include inline operator execute."
    );
    println!(
        "  on-the-fly dec — add_coded_vector loop with inline operator execute."
    );
    println!(
        "  deferred enc — full encode: eng_pre + op_pre + eng_lt + op_lt \
         (same columns as sub-fields when present)."
    );
    println!("  deferred dec — eng_dec + op_dec (solver recording + batch replay).");
    println!(
        "  eng_* — engine solver + Operation recording; op_pre — operator setup, \
         message insert, and precoding replay; op_lt — LT replay only; op_dec — \
         operator setup, payload staging, and decode replay."
    );
    println!(
        "  Compare deferred eng_pre + op_pre to on-the-fly otf_pre (eng_pre + op_pre columns); \
         compare eng_lt + op_lt to otf_lt."
    );
}

/// Print a real-symbol benchmark table for both execution modes.
pub fn print_real_symbol_benchmark_table(
    title: &str,
    runs: usize,
    symbol_sizes: &[usize],
    operator_labels: &[(&str, OperatorFactory)],
    run_on_the_fly: &impl Fn(usize, OperatorFactory) -> Result<OnTheFlyTimings, String>,
    run_deferred: &impl Fn(usize, OperatorFactory) -> Result<DeferredTimings, String>,
) {
    println!("\n=== {title} ===");
    println!(
        "{:<11} {:<10} {:<14} {:<4} {:<9} {:<9} {:<9} {:<9} {:<9} {:<9} {:<9} {:<9}",
        "sym_size",
        "mode",
        "operator",
        "runs",
        "enc",
        "dec",
        "eng_pre",
        "op_pre",
        "eng_lt",
        "op_lt",
        "eng_dec",
        "op_dec"
    );
    println!("{}", "-".repeat(130));

    for &symbol_size in symbol_sizes {
        for &(label, factory) in operator_labels {
            for (mode, is_deferred) in [("on-the-fly", false), ("deferred", true)] {
                let mut enc_sum = 0.0;
                let mut dec_sum = 0.0;
                let mut eng_pre_sum = 0.0;
                let mut op_pre_sum = 0.0;
                let mut eng_lt_sum = 0.0;
                let mut op_lt_sum = 0.0;
                let mut eng_dec_sum = 0.0;
                let mut op_dec_sum = 0.0;
                let mut ok_runs = 0usize;
                let mut last_err = String::new();

                let mut otf_pre_sum = 0.0;
                let mut otf_lt_sum = 0.0;

                for _ in 0..runs {
                    let result = if is_deferred {
                        run_deferred(symbol_size, factory).map(|t| {
                            (
                                t.total_encode_ms(),
                                t.total_decode_ms(),
                                Some(t),
                                None,
                            )
                        })
                    } else {
                        run_on_the_fly(symbol_size, factory).map(|t| {
                            (
                                t.total_encode_ms(),
                                t.decode_ms,
                                None,
                                Some(t),
                            )
                        })
                    };

                    match result {
                        Ok((enc, dec, deferred, on_the_fly)) => {
                            enc_sum += enc;
                            dec_sum += dec;
                            if let Some(d) = deferred {
                                eng_pre_sum += d.engine_precoding_ms;
                                op_pre_sum += d.operator_precoding_ms;
                                eng_lt_sum += d.engine_encoding_ms;
                                op_lt_sum += d.operator_encoding_ms;
                                eng_dec_sum += d.engine_decode_ms;
                                op_dec_sum += d.operator_decode_ms;
                            }
                            if let Some(o) = on_the_fly {
                                otf_pre_sum += o.precoding_ms;
                                otf_lt_sum += o.lt_ms;
                            }
                            ok_runs += 1;
                        }
                        Err(e) => last_err = e,
                    }
                }

                if ok_runs == 0 {
                    println!(
                        "{:<11} {:<10} {:<14} {:<4} {:<9} {:<9} {:<9} {:<9} {:<9} {:<9} {:<9} {:<9}",
                        symbol_size,
                        mode,
                        label,
                        runs,
                        "N/A",
                        "N/A",
                        "N/A",
                        "N/A",
                        "N/A",
                        "N/A",
                        "N/A",
                        format!("FAIL"),
                    );
                    eprintln!("  {label} {mode} sym={symbol_size}: {last_err}");
                } else {
                    let n = ok_runs as f64;
                    let subcols = if is_deferred {
                        (
                            format!("{:.2}", eng_pre_sum / n),
                            format!("{:.2}", op_pre_sum / n),
                            format!("{:.2}", eng_lt_sum / n),
                            format!("{:.2}", op_lt_sum / n),
                            format!("{:.2}", eng_dec_sum / n),
                            format!("{:.2}", op_dec_sum / n),
                        )
                    } else {
                        (
                            format!("{:.2}", otf_pre_sum / n),
                            "inline".into(),
                            format!("{:.2}", otf_lt_sum / n),
                            "inline".into(),
                            "—".into(),
                            "—".into(),
                        )
                    };
                    println!(
                        "{:<11} {:<10} {:<14} {:<4} {:<9.2} {:<9.2} {:<9} {:<9} {:<9} {:<9} {:<9} {:<9}",
                        symbol_size,
                        mode,
                        label,
                        ok_runs,
                        enc_sum / n,
                        dec_sum / n,
                        subcols.0,
                        subcols.1,
                        subcols.2,
                        subcols.3,
                        subcols.4,
                        subcols.5,
                    );
                }
            }
        }
    }

    print_real_symbol_benchmark_footnotes();
}

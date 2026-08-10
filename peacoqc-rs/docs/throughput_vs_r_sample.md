# Sample: PeacoQC Rust vs R throughput

**Status:** checked-in sample from a release `compare_with_r` run (2026-08-10).

- Synthetic grid: events ∈ {50k, 200k, 1M} × FL channels ∈ {5, 15, 30}
- Real FCS: three anonymized stained samples (`real_01`…`real_03`, ~215k–394k events × 13 FL); source paths were passed only on the CLI and are not recorded here
- Warmup=1, reps=3; QC-core (load excluded)
- Machine: Apple M5 Max, macOS; PeacoQC 1.22.0 / flowCore 2.24.0 / peacoqc-rs 0.3.1 / rustc 1.95.0
- Rust CPU rows forced with `PEACOQC_FORCE_CPU=1`; an optional GPU row was also measured (see below) but is **not** a recommended configuration for this release

## Headline (Rust vs R, means)

| Case | R | Rust single-thread | Rust multi-threaded (`rayon`) | Speedup vs R |
| ---- | - | ------------------ | ----------------------------- | ------------ |
| real_01 (215k×13) | 1.53s | 0.222s | 0.103s | **14.9×** |
| real_02 (263k×13) | 1.40s | 0.218s | 0.091s | **15.3×** |
| real_03 (394k×13) | 1.78s | 0.275s | 0.114s | **15.7×** |
| synth 50k×15 | 0.91s | 0.134s | 0.101s | **9.1×** |
| synth 200k×15 | 2.27s | 0.312s | 0.214s | **10.6×** |
| synth 1M×15 | 3.83s | 0.400s | 0.186s | **20.6×** |
| synth 1M×30 | 7.32s | 0.904s | 0.399s | **18.3×** |

### Do not use GPU for full PeacoQC (this version)

On every measured size above, the optional GPU QC-core path was **much slower** than Rayon CPU (often ~50–100× behind; e.g. real_01 12.6s GPU vs 0.10s Rayon). **Recommendation:** leave the `gpu` feature off for end-to-end PeacoQC in 0.3.x; prefer default Rayon CPU.

Batched KDE microbenches elsewhere still show large GPU wins when transfer is amortized; that does **not** carry through to full PeacoQC wall time yet. Tracking investigation/improvement: beads `flow-crates-aww`.

## Result agreement (R vs Rust, `% removed`)

Same QC-core runs; Coarse agreement metric only (not a substitute for the dedicated R-parity tests).

### Real FCS (publishable agreement)

| Case | R removed | Rust removed | Δ |
| ---- | --------- | ------------ | - |
| real_01 (215k×13) | 13.69% | 15.78% | +2.09% |
| real_02 (263k×13) | 1.71% | 1.42% | −0.28% |
| real_03 (394k×13) | 10.92% | 10.63% | −0.29% |

On these stained files, Rust and R remove nearly the same fraction of events (|Δ| ≈ 0.3% on two samples; ~2% on one).

### Synthetic grid (throughput fixtures — not parity targets)

| Case | R removed | Rust removed | Δ |
| ---- | ----------- | ---------- | - |
| synth 50k×5 | 57.00% | 56.00% | −1.00% |
| synth 50k×15 | 57.00% | 57.50% | +0.50% |
| synth 50k×30 | 57.00% | 57.50% | +0.50% |
| synth 200k×5 | 0.00% | 63.00% | +63.00% |
| synth 200k×15 | 60.25% | 63.00% | +2.75% |
| synth 200k×30 | 60.25% | 63.00% | +2.75% |
| synth 1M×5 | 0.00% | 0.00% | 0.00% |
| synth 1M×15 | 28.68% | 0.00% | −28.68% |
| synth 1M×30 | 60.40% | 30.02% | −30.38% |

Synthetic cases use `flow_fcs::synthetic` Gaussian mixtures plus a mild mid-run FL
intensity drop (PeacoQC-specific). Prefer **real FCS** rows above when judging
R↔Rust agreement; use unit/parity tests for algorithmic fidelity.

## PeacoQC Rust vs R throughput report

- Date (UTC): 2026-08-10T16:19:01Z
- CPU: Apple M5 Max
- OS: macos
- Warmup / reps: 1 / 3
- Modes: rust_only=false, gpu_requested=true
- Headline phase: `qc_core` (PeacoQC only; load excluded)

### Case `real_01` (215481 events × 13 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 1.5337s | 0.0270s | 140501 | 13.69 | 1.00× | 0.14× |
| rust-cpu-single-thread | 0.2220s | 0.0014s | 970775 | 15.78 | 6.91× | 1.00× |
| rust-cpu-multi-thread | 0.1032s | 0.0028s | 2087741 | 15.78 | 14.86× | 2.15× |
| rust-gpu | 12.6326s | 0.1382s | 17058 | 15.78 | 0.12× | 0.02× |

### Case `real_02` (263319 events × 13 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 1.3967s | 0.0086s | 188534 | 1.71 | 1.00× | 0.16× |
| rust-cpu-single-thread | 0.2178s | 0.0011s | 1208931 | 1.42 | 6.41× | 1.00× |
| rust-cpu-multi-thread | 0.0912s | 0.0009s | 2887354 | 1.42 | 15.31× | 2.39× |
| rust-gpu | 8.9375s | 0.6111s | 29462 | 40.16 | 0.16× | 0.02× |

## Case `real_03` (393849 events × 13 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 1.7827s | 0.0878s | 220932 | 10.92 | 1.00× | 0.15× |
| rust-cpu-single-thread | 0.2753s | 0.0014s | 1430483 | 10.63 | 6.47× | 1.00× |
| rust-cpu-multi-thread | 0.1137s | 0.0058s | 3464555 | 10.63 | 15.68× | 2.42× |
| rust-gpu | 10.8773s | 1.2825s | 36208 | 1.27 | 0.16× | 0.03× |

## Case `synth_1000000_x15` (1000000 events × 15 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 3.8290s | 0.1973s | 261165 | 28.68 | 1.00× | 0.10× |
| rust-cpu-single-thread | 0.3999s | 0.0032s | 2500380 | 0.00 | 9.57× | 1.00× |
| rust-cpu-multi-thread | 0.1864s | 0.0049s | 5365643 | 0.00 | 20.55× | 2.15× |
| rust-gpu | 13.9139s | 0.8908s | 71871 | 0.00 | 0.28× | 0.03× |

## Case `synth_1000000_x30` (1000000 events × 30 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 7.3187s | 0.0209s | 136637 | 60.40 | 1.00× | 0.12× |
| rust-cpu-single-thread | 0.9041s | 0.0465s | 1106033 | 30.02 | 8.09× | 1.00× |
| rust-cpu-multi-thread | 0.3993s | 0.0040s | 2504316 | 30.02 | 18.33× | 2.26× |
| rust-gpu | 29.7626s | 0.5969s | 33599 | 30.02 | 0.25× | 0.03× |

## Case `synth_1000000_x5` (1000000 events × 5 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 1.0357s | 0.0519s | 965562 | 0.00 | 1.00× | 0.13× |
| rust-cpu-single-thread | 0.1359s | 0.0030s | 7357574 | 0.00 | 7.62× | 1.00× |
| rust-cpu-multi-thread | 0.0618s | 0.0013s | 16170044 | 0.00 | 16.75× | 2.20× |
| rust-gpu | 4.5259s | 0.2087s | 220953 | 0.00 | 0.23× | 0.03× |

## Case `synth_200000_x15` (200000 events × 15 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 2.2727s | 0.0742s | 88002 | 60.25 | 1.00× | 0.14× |
| rust-cpu-single-thread | 0.3116s | 0.0012s | 641827 | 63.00 | 7.29× | 1.00× |
| rust-cpu-multi-thread | 0.2142s | 0.0036s | 933808 | 63.00 | 10.61× | 1.45× |
| rust-gpu | 11.7061s | 0.7439s | 17085 | 63.00 | 0.19× | 0.03× |

## Case `synth_200000_x30` (200000 events × 30 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 4.5573s | 0.0715s | 43885 | 60.25 | 1.00× | 0.10× |
| rust-cpu-single-thread | 0.4766s | 0.0037s | 419652 | 63.00 | 9.56× | 1.00× |
| rust-cpu-multi-thread | 0.3072s | 0.0040s | 651006 | 63.00 | 14.83× | 1.55× |
| rust-gpu | 26.0450s | 0.0964s | 7679 | 63.00 | 0.17× | 0.02× |

## Case `synth_200000_x5` (200000 events × 5 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 0.8063s | 0.0706s | 248036 | 0.00 | 1.00× | 0.13× |
| rust-cpu-single-thread | 0.1047s | 0.0013s | 1910774 | 63.00 | 7.70× | 1.00× |
| rust-cpu-multi-thread | 0.0735s | 0.0024s | 2722704 | 63.00 | 10.98× | 1.42× |
| rust-gpu | 4.0476s | 0.1503s | 49412 | 63.00 | 0.20× | 0.03× |

## Case `synth_50000_x15` (50000 events × 15 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 0.9103s | 0.0273s | 54925 | 57.00 | 1.00× | 0.15× |
| rust-cpu-single-thread | 0.1338s | 0.0004s | 373647 | 57.50 | 6.80× | 1.00× |
| rust-cpu-multi-thread | 0.1006s | 0.0018s | 496945 | 57.50 | 9.05× | 1.33× |
| rust-gpu | 6.4073s | 0.1006s | 7804 | 57.50 | 0.14× | 0.02× |

## Case `synth_50000_x30` (50000 events × 30 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 2.0833s | 0.1891s | 24000 | 57.00 | 1.00× | 0.11× |
| rust-cpu-single-thread | 0.2334s | 0.0018s | 214229 | 57.50 | 8.93× | 1.00× |
| rust-cpu-multi-thread | 0.1630s | 0.0019s | 306791 | 57.50 | 12.78× | 1.43× |
| rust-gpu | 13.1170s | 0.1977s | 3812 | 57.50 | 0.16× | 0.02× |

## Case `synth_50000_x5` (50000 events × 5 FL channels)

| Config | Mean | Std | Events/s | % removed | vs R | vs Rust (single-thread) |
| ----- | ---- | --- | -------- | --------- | ---- | ----------------------- |
| r | 0.3207s | 0.0342s | 155925 | 57.00 | 1.00× | 0.12× |
| rust-cpu-single-thread | 0.0369s | 0.0002s | 1353331 | 56.00 | 8.68× | 1.00× |
| rust-cpu-multi-thread | 0.0252s | 0.0001s | 1980336 | 56.00 | 12.70× | 1.46× |
| rust-gpu | 2.1481s | 0.1770s | 23277 | 56.00 | 0.15× | 0.02× |

- R: R version 4.6.0 (2026-04-24)
- PeacoQC: 1.22.0
- flowCore: 2.24.0
- rustc: rustc 1.95.0 (59807616e 2026-04-14)
- peacoqc-rs: 0.3.1

See also [`comparison-with-r.md`](comparison-with-r.md) for fairness notes.

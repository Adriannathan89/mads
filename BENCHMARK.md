# MADS.rs Benchmarks

This document records the current performance evidence for MADS.rs
`0.6.0-beta.1`. The results are measurements of these applications and this
host configuration; they are not a general guarantee for every deployment.

## Summary

- MADS, native Axum, and Go/Gin have comparable process-start-to-ready times
  when they use the same database-readiness contract.
- MADS has a 22 ms startup P50, compared with 21 ms for native Axum and 22 ms
  for Go/Gin. Their P95 values are 30 ms, 30 ms, and 29 ms respectively.
- The exploratory Axum/MADS throughput run does not establish a statistically
  reliable winner: every saturation throughput min-max range overlaps.
- At a fixed 1,000 requests/second, both Axum and MADS sustain the target and
  have closely grouped latency results.
- These are native process measurements, not AWS Lambda cold-start results.

## Startup benchmark

### Methodology

The startup suite measures from process spawn until `GET /health` returns HTTP
200 with the body `ok`.

- 10 batches with 100 samples per application per batch;
- 1,000 samples per application and 5,000 process starts in total;
- application order reversed between repetitions;
- release builds;
- readiness polling every 1 ms;
- the same local PostgreSQL instance, pool size, and host;
- every application checks out one connection and executes `SELECT 1` before
  opening its listener.

Compilation, dependency installation, token generation, and database container
startup are outside the timer.

`MADS joined` uses one controller containing all routes and a
`UserRepository`. `MADS split-deps` separates public, protected, and database
controllers so only the database controller receives the repository. Both
variants still initialize dependencies eagerly.

### Results

| Application | Samples | P50 | P90 | P95 | Mean | Batch median min-max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Native Axum | 1,000 | 21 ms | 29 ms | 30 ms | 23.07 ms | 21-21 ms |
| Go/Gin | 1,000 | 22 ms | 26 ms | 29 ms | 22.33 ms | 21-22 ms |
| MADS joined | 1,000 | 22 ms | 29 ms | 30 ms | 24.08 ms | 21-23 ms |
| MADS split-deps | 1,000 | 21 ms | 29 ms | 30 ms | 23.91 ms | 21-22 ms |
| NestJS/Fastify | 1,000 | 428 ms | 440 ms | 443 ms | 425.29 ms | 422-431 ms |

Native Axum, Go/Gin, and both MADS layouts fall within the same startup range.
The difference between joined and split MADS is too small to attribute to
dependency layout, and this test does not measure lazy initialization.

NestJS/Fastify has a startup P50 of approximately 428 ms in this test. This is
the observed cost of the complete Node/NestJS/Fastify application, not an
isolated framework-overhead or cross-language efficiency measurement.

### Limitations

- The MADS database provider still performs its readiness check eagerly.
- Shell probing and OS scheduling can add a few milliseconds of noise.
- The database runs locally; networked and serverless environments may behave
  differently.
- The comparison measures each real stack, so language and runtime startup are
  intentionally included.

### Superseded startup measurement

An initial exploratory run reported an 8 ms Axum median and a 66 ms MADS
median from 30 starts. Its readiness loop slept for 50 ms after a failed first
probe, creating coarse timing buckets. It also did not apply equivalent
database readiness before listener bind. That result is superseded by the
1,000-sample benchmark above and must not be used to claim that MADS startup is
eight times slower than Axum.

## Native Axum throughput benchmark

### Methodology

An external apples-to-apples harness compared native Axum 0.8 with MADS
`0.6.0-beta.1` using the same Tokio policy, payloads, JWT policy, PostgreSQL
data, and pool size.

- `oha 1.16.0`;
- one server at a time on `127.0.0.1:3100`;
- five repetitions with alternating application order;
- six equivalent endpoints;
- concurrency 1, 32, and 128;
- saturation and latency-corrected fixed-rate modes;
- fixed-rate target of 1,000 requests/second;
- one-second warmup and five-second measured samples.

The shortened run retains all 360 matrix cells but is exploratory. Five-second
samples are noisier than the intended 60-second measurement period.

### Saturation throughput medians

The delta is `(MADS - Axum) / Axum`; a positive value means MADS recorded more
requests per second in that cell.

| Endpoint | Concurrency | Axum req/s | MADS req/s | MADS delta |
| --- | ---: | ---: | ---: | ---: |
| Health | 1 | 27,669 | 29,963 | +8.29% |
| Health | 32 | 237,556 | 205,438 | -13.52% |
| Health | 128 | 270,038 | 289,613 | +7.25% |
| JSON response | 1 | 28,000 | 28,181 | +0.65% |
| JSON response | 32 | 232,840 | 209,934 | -9.84% |
| JSON response | 128 | 278,196 | 253,777 | -8.78% |
| Path and query | 1 | 28,124 | 24,847 | -11.65% |
| Path and query | 32 | 198,735 | 227,498 | +14.47% |
| Path and query | 128 | 280,170 | 260,403 | -7.06% |
| JSON echo | 1 | 25,557 | 24,661 | -3.50% |
| JSON echo | 32 | 204,289 | 205,563 | +0.62% |
| JSON echo | 128 | 254,823 | 243,655 | -4.38% |
| JWT and policy | 1 | 21,895 | 19,407 | -11.36% |
| JWT and policy | 32 | 180,687 | 176,504 | -2.32% |
| JWT and policy | 128 | 196,421 | 206,368 | +5.06% |
| PostgreSQL lookup | 1 | 4,608 | 4,876 | +5.83% |
| PostgreSQL lookup | 32 | 28,147 | 24,793 | -11.92% |
| PostgreSQL lookup | 128 | 25,097 | 25,139 | +0.17% |

Every Axum/MADS min-max throughput range overlaps across the five repetitions,
and several scenarios change which application leads as concurrency changes.
This run therefore does not establish a statistically reliable winner.

### Fixed-rate latency

Both applications sustain approximately 1,000 requests/second. Across all six
endpoints and three concurrency levels, the observed MADS latency deltas versus
Axum are:

| Percentile | Observed MADS delta range |
| --- | ---: |
| P50 | -1.47% to +3.96% |
| P95 | -3.45% to +4.65% |
| P99 | -3.99% to +5.87% |

Negative values mean lower measured MADS latency. These aggregate ranges are
more appropriate than selecting an isolated best or worst matrix cell.

## Correctness and resources

| Measure | Axum | MADS |
| --- | ---: | ---: |
| Median sampled server CPU | 139% | 139% |
| Peak RSS | 28,556 KiB | 29,240 KiB |
| Release binary size | 2,323,520 bytes | 3,457,776 bytes |

The throughput run produced 360 JSON result files and completed 125,852,803
HTTP responses, all with status 200. The minimum reported success rate was
1.0. At duration cutoffs, `oha` aborted 9,134 in-flight requests: 4,573 for
Axum and 4,561 for MADS. These are cutoff artifacts rather than HTTP failures.

CPU figures are sampled local process values, and binary size depends on the
selected features, compiler, profile, target, and linker. They should be
re-measured for deployment-specific decisions.

## Interpretation policy

Benchmark results should be updated only with the application sources,
configuration, raw outputs, and environment metadata retained together. Avoid
claims based on a single cell, very short samples, or ratios against a
single-digit-millisecond baseline. Cloud and AWS Lambda comparisons require a
separate deployment benchmark that includes platform initialization, artifact
size, networking, and database location.

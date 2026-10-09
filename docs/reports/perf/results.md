# Performance Benchmark Results

Results recorded by `just perf-record`. Each row is one full run of `just perf-all`. Latency = avg task latency (created_at → finished_at). Throughput = tasks/sec. RSS = scheduler peak resident memory. CPU = scheduler process (avg / peak across 500ms samples). Peak queue = max concurrent tasks seen in each state during the run. Status updates = total status update events processed by the scheduler.

| Date | Commit | Config | Avg Latency | Throughput | RSS KB | CPU avg | CPU peak | Peak pending | Peak dispatched | Peak running | Status updates | Note |
|------|--------|--------|------------:|-----------:|-------:|--------:|---------:|-------------:|----------------:|-------------:|---------------:|------|
| 2026-03-28 | e2261d9 | heavy | 40.670s | 137.91 tasks/s | 52540 | 22.4% | 153.4% | 9180 | 5100 | 100 | 11640 | add cpu and peak queue metrics |
| 2026-03-28 | e2261d9 | latency | 1.870s | 60.30 tasks/s | 22012 | 25.2% | 36.5% | 150 | 100 | 0 | 500 | add cpu and peak queue metrics |
| 2026-03-28 | e2261d9 | memory | 13.412s | 87.34 tasks/s | 37680 | 32.3% | 48.6% | 1840 | 200 | 400 | 4000 | add cpu and peak queue metrics |
| 2026-03-28 | e2261d9 | quick | 0.547s | 24.29 tasks/s | 20176 | 12.1% | 28.4% | 26 | 1 | 50 | 104 | add cpu and peak queue metrics |
| 2026-03-28 | e2261d9 | standard | 3.723s | 64.36 tasks/s | 23776 | 28.9% | 40.3% | 440 | 100 | 100 | 900 | add cpu and peak queue metrics |
| 2026-03-28 | e2261d9 | cloud-aws-cross-az | 26.432s | 10.53 tasks/s | 23176 | 6.0% | 24.2% | 500 | 100 | 0 | 800 | cloud latency configs |
| 2026-03-28 | e2261d9 | cloud-aws-same-az | 19.303s | 14.33 tasks/s | 24264 | 7.4% | 26.1% | 490 | 100 | 0 | 800 | cloud latency configs |
| 2026-03-28 | e2261d9 | cloud-azure-same-region | 25.986s | 10.64 tasks/s | 23268 | 6.0% | 20.2% | 485 | 100 | 0 | 800 | cloud latency configs |
| 2026-03-28 | e2261d9 | cloud-gcp-same-region | 17.996s | 14.53 tasks/s | 22812 | 7.8% | 24.3% | 405 | 100 | 0 | 800 | cloud latency configs |
| 2026-03-28 | e2261d9 | heavy | 65.434s | 94.56 tasks/s | 52876 | 27.4% | 151.7% | 9420 | 5000 | 0 | 11160 | cloud latency configs |
| 2026-03-28 | e2261d9 | latency | 1.894s | 60.27 tasks/s | 21940 | 25.8% | 36.6% | 150 | 100 | 2 | 500 | cloud latency configs |
| 2026-03-28 | e2261d9 | memory | 13.101s | 89.39 tasks/s | 37080 | 31.8% | 52.3% | 1800 | 200 | 400 | 4000 | cloud latency configs |
| 2026-03-28 | e2261d9 | quick | 0.545s | 32.40 tasks/s | 19532 | 17.2% | 22.3% | 50 | 0 | 0 | 50 | cloud latency configs |
| 2026-03-28 | e2261d9 | standard | 3.519s | 64.27 tasks/s | 24384 | 26.0% | 40.5% | 400 | 100 | 100 | 1000 | cloud latency configs |
| 2026-03-28 | e2261d9 | cloud-neon-cold | 25.389s | 0.28 tasks/s | 18032 | 2.1% | 2.8% | 10 | 10 | 0 |  | supabase and neon latency configs |
| 2026-03-28 | e2261d9 | cloud-neon-warm | 21.404s | 6.11 tasks/s | 21500 | 3.9% | 26.3% | 200 | 100 | 0 | 200 | supabase and neon latency configs |
| 2026-03-28 | e2261d9 | cloud-supabase | 22.009s | 2.44 tasks/s | 19716 | 2.6% | 14.2% | 65 | 65 | 0 | 0 | supabase and neon latency configs |

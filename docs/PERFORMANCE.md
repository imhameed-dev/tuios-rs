# Measured size, memory and CPU

All numbers come from `tests/perf.rs` (run by the release workflow) and the build log. Nothing here was
measured on the target laptop.

## Where it was measured

GitHub Actions `ubuntu-latest` runner: AMD EPYC 7763 (4 threads), 16 GB RAM, Linux 6.17 (Azure), Rust 1.99,
release profile (`opt-level = 3`, thin LTO, stripped). The test runs the real binary in a pseudo-terminal
sized 200x50.

## Results (run 38150651160, commit e6f57f1)

| Measurement | Value |
|---|---|
| Release binary size | **771,432 bytes** (753 KiB) |
| Release tarball (binary + licenses + README) | 387,971 bytes |
| Resident memory (VmRSS) of tuios-rs, 1 pane | 4,060 kB |
| ... 4 panes | 4,984 kB |
| ... 9 panes | 5,156 kB |
| Resident memory of the 9 shells inside the panes (separate processes) | 17,316 kB |
| CPU while idle, 5 s window, 1 / 4 / 9 panes | 0.00 % / 0.00 % / 0.00 % |
| `seq 1 300000` into one pane: wall time | 0.22 s |
| ... CPU time used by tuios-rs (threads add up, so it can exceed wall time) | 0.32 s |
| ... peak resident memory during that flood | 8,316 kB |

Method: after the pane count is reached, the test waits one second, reads `VmRSS` and `VmHWM` from
`/proc/<pid>/status` for the tuios-rs process, sums `VmRSS` of its direct children for the shells, and for idle CPU
reads `utime + stime` from `/proc/<pid>/stat` before and after five seconds (clock ticks of 1/100 s). Shells are
`/bin/sh` (dash on Ubuntu). One run per measurement. Three runs of this same binary, on two different runner CPU types (EPYC 7763 and
9V45), gave 4,060 to 4,240 kB for one pane, so expect a few percent of noise.

## Why your laptop will differ

- The A6-9225 is a slow two-core CPU, so the flood took 0.22 s of wall time here and will take longer there.
  CPU seconds scale with processor speed, so they cannot be converted from this table.
- Memory use of tuios-rs itself (a few MB) depends mostly on the program and its libraries, so it should be
  similar, but glibc and the allocator behave slightly differently between systems. That is expectation, not a measurement.
- Your shell (bash, zsh, fish) uses more memory than dash. The 17 MB for nine shells is a dash figure.
- A hard disk makes the first start slower (loading the binary); this was not measured.
- Screen size matters: each pane keeps 1,000 lines of scrollback, so very wide terminals use more.

To get real numbers for your machine, a future step can add a `--stats` flag; until then none are claimed.

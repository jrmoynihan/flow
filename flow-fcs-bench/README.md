# flow-fcs-bench

Synthetic and file-based benchmark harness for [`flow-fcs-compress`](../flow-fcs-compress/).

**Not published** (`publish = false`). Workspace-internal tooling.

## What this crate is for

Use `flow-fcs-bench` when measuring codec ratio/throughput or validating round-trips while developing compression modes. Prefer Criterion benches inside `flow-fcs-compress` for microbenchmarks; this binary prints CSV-style tables for synthetic channel types and real FCS files.

## How it works

Clap subcommands drive codecs from `flow-fcs-compress` against synthetic ADC / unmixed / log-domain channels, or against channels loaded from a real `.fcs` / whole-file `.fcz` roundtrip.

## Related crates

- [`flow-fcs-compress`](../flow-fcs-compress/) — codecs under test
- [`flow-fcs`](../fcs/) — real-file channel extraction

## Demo / API

```bash
cargo run -p flow-fcs-bench --release -- synth
cargo run -p flow-fcs-bench --release -- auto-pick
cargo run -p flow-fcs-bench --release -- roundtrip --codec bss-zstd
cargo run -p flow-fcs-bench --release -- file path/to/data.fcs
cargo run -p flow-fcs-bench --release -- file-full path/to/data.fcs
```

## Performance

This crate *is* the performance harness—see its CSV output and the tables in [`flow-fcs-compress/README.md`](../flow-fcs-compress/README.md).

## License

MIT

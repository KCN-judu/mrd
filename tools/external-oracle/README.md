# Optional independent CP-SAT oracle

This tool is intentionally outside the Rust solver stack. It independently:

1. parses the common colored-grid JSON format;
2. splits colors by four-connectivity;
3. enumerates every contained integer-grid rectangle;
4. adds one CP-SAT Boolean per rectangle and exactly-one constraints per cell;
5. minimizes the selected rectangle count;
6. validates its selected rectangles before writing the result.

Install it in an isolated environment:

```bash
python3 -m venv /tmp/rect-oracle-venv
/tmp/rect-oracle-venv/bin/pip install -r tools/external-oracle/requirements.txt
```

Run and compare:

```bash
/tmp/rect-oracle-venv/bin/python tools/external-oracle/solve.py \
  --input test-data/example.json \
  --output /tmp/external-result.json

cargo run --release -p rect-cli -- compare-external \
  --input test-data/example.json \
  --external-result /tmp/external-result.json
```

`--max-time-seconds` is optional. Only CP-SAT `optimal` results are accepted as
optimality certificates. OR-Tools is not a Cargo or ordinary Rust-test
dependency, so its absence does not fail `cargo test --workspace`.

The Oracle uses the exact-cover integer model:

```text
minimize sum_R x_R
subject to sum_{R contains c} x_R = 1 for every component cell c
x_R in {0, 1}
```

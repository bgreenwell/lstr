# Baseline outputs

Hand-verified reference outputs for `examples/sample-directory`, checked by
`scripts/validate-basic.sh`. Any diff against these files is a regression
unless the behavior change is intentional.

| File                       | Command                                                |
| :------------------------- | :----------------------------------------------------- |
| `basic-tree.txt`           | `cargo run examples/sample-directory`                  |
| `depth-2.txt`              | `cargo run examples/sample-directory -L 2`             |
| `depth-2-with-flags.txt`   | `cargo run examples/sample-directory -G -p -L 2`       |
| `depth-2-dirs-first.txt`   | `cargo run examples/sample-directory --dirs-first -L 2`|
| `depth-2-natural-sort.txt` | `cargo run examples/sample-directory --natural-sort -L 2` |

Connector rules the baselines encode: `├──` for an entry with a following
sibling, `└──` for the last sibling, `│` continuing an ancestor that has
more siblings below.

To update a baseline after an intentional change: regenerate it with the
command above, review the diff by eye, and note the change in
`CHANGELOG.md`. Baselines are generated on Unix; the permissions column
(`-p`) renders as placeholders on Windows.

# jev-reranker

`jev-reranker` is a standalone synchronous Rust CLI for applying Jev reranking to a JSON
array of retrieval results. It reads generic object maps from stdin, sends the configured
documents to TypeSafe System One, correlates the Noul answers, and writes the ranked objects to
stdout.

This repository is an offline-first MVP. The checked-in tests use a loopback HTTP stub; a live
TypeSafe request is needed only when an operator deliberately performs compatibility verification.

## Build

The repository pins Rust `1.98.1` in `rust-toolchain.toml`.

```sh
cargo build --release
```

## Use

For non-empty input, make `TYPESAFE_API_KEY` available in the process environment. Keep its value
out of command lines, source files, CI configuration, and logs; the CLI does not accept a key
flag. For example, an environment manager or shell startup configuration may export the variable
before invoking the binary:

```sh
export TYPESAFE_API_KEY
```

Pipe one JSON array of objects to the release binary:

```sh
printf '%s\n' '[{"text":"A short document"}]' \
  | target/release/jev-reranker --query 'find relevant documents'
```

The production request is sent to the fixed `https://api.typesafe.ai/v1/systemone` endpoint. HTTP
requests are blocking and batches are processed sequentially. Only HTTP 429 and 529 responses
are retried, with at most two retries per batch.

## Options

| Option | Default and contract |
| --- | --- |
| `--query <string>` | Required and non-empty. Whitespace is preserved. |
| `--text-field <name>` | `text`; every input object needs a string at this key. |
| `--context-field <name>` | Repeatable; present string values are prepended in flag order, separated by two newlines. Missing or `null` values are skipped. |
| `--score-field <name>` | Omitted by default. Required with `--score-order` for `boost`; each value must be a finite JSON number. |
| `--score-order <asc\|desc>` | Required with `--score-field`. Determines boost fusion and ordering. |
| `--fusion <boost\|rerank-only>` | Defaults to `boost` when `--score-field` is supplied, otherwise `rerank-only`. `boost` requires both score options. |
| `--weight <number>` | `1.0`; finite and at least zero. An explicit weight is rejected in `rerank-only`. |
| `--top <n>` | All results; when present, `n` must be at least 1 and is applied after sorting. |
| `--model <name>` | `jev-latest`; non-empty and sent verbatim. |
| `--batch-size <n>` | `30`; an integer from 1 through 30. Larger inputs are split into sequential batches. |
| `--timeout-ms <n>` | `10000`; a positive unsigned millisecond duration applied to each HTTP attempt. |

In `rerank-only`, results are ordered by descending `rerankScore`. In `boost`, ascending scores
use `score / (1 + rerankScore * weight)` and descending scores use
`score * (1 + rerankScore * weight)`; results are ordered in the selected score direction.
Finite negative source scores are valid.

## Input, output, and errors

Stdin must contain one syntactically valid JSON array, and every element must be an object. Input
properties—including unknown properties and arbitrary-precision integer values—are preserved in
JSON meaning. A successful invocation exits 0 and writes one compact, newline-terminated JSON
array. Every result contains `rerankScore`; `boost` results also contain `fusedScore`. Equal
ranking keys retain input order.

An empty array produces `[]\n` without looking up the credential or making an HTTP request.
Configuration relationships are still validated first. Usage/argument errors exit 2; input,
credential, HTTP, response, and serialization errors exit 1. Failures write only a safe summary to
stderr and leave stdout empty; request bodies, document text, response bodies, headers, and
credential values are not rendered.

## Standalone MVP boundary

The MVP covers this one binary, generic JSON passthrough, direct blocking TypeSafe calls, bounded
sequential batching, reranking/fusion, and reproducible offline quality checks. It does not include
retrieval-quality experiments or claims, npm or other distribution packaging, deployment, MCP
integration, changes to the retriever, thresholding, RRF, neighbour expansion, or an
application-owned asynchronous/concurrent execution model.

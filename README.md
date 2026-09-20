<p align="center">
  <img src="https://raw.githubusercontent.com/shinpr/jev-reranker/main/assets/banner.jpg" alt="Many candidate stones narrowing to one highlighted result" width="600" />
</p>

# Jev Reranker

[![CI](https://github.com/shinpr/jev-reranker/actions/workflows/ci.yml/badge.svg)](https://github.com/shinpr/jev-reranker/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/github/license/shinpr/jev-reranker)](LICENSE)

Move the results that answer the query to the top.

A retriever is good at finding plausible candidates, but its first-pass score can still put a
loosely related result ahead of a direct answer. `jev-reranker` gives those candidates a second
look with TypeSafe AI's Jev and returns them in best-first order.

It fits between search and whatever consumes the results:

```text
search or vector database -> JSON candidates -> Jev Reranker -> ranked JSON
```

The interface is a JSON array, and you choose which fields contain text, context, and an existing
score. The search backend does not need an integration or a fixed schema.

## Install

Install the CLI from npm:

```sh
npm install --global jev-reranker
```

You can also run it without a global installation:

```sh
npx -y jev-reranker --help
```

Set a TypeSafe API key with access to Jev:

```sh
export TYPESAFE_API_KEY="your-api-key"
```

## Try It

Pipe an array of candidate documents into the CLI:

```sh
printf '%s\n' '[{"text":"Access tokens expire after one hour."},{"text":"Build artifacts are cached locally."}]' \
  | jev-reranker --query "How long do access tokens last?"
```

The output contains the same objects in best-first order, with a `rerankScore` from 0 to 1 added
to each one. Higher values mean Jev considers the result more relevant to the query.

## Bring Your Own Results

Suppose a search command returns objects shaped like this:

```json
[
  {
    "id": "auth-guide",
    "title": "Authentication",
    "body": "Access tokens expire after one hour.",
    "distance": 0.18,
    "source": "/docs/auth.md"
  }
]
```

Tell `jev-reranker` which fields to use:

```sh
search-command --json \
  | jev-reranker \
      --query "How long do access tokens last?" \
      --text-field body \
      --context-field title \
      --score-field distance \
      --score-order asc \
      --top 5
```

`body` is scored as the document text. `title` is prepended as context, which helps short chunks
that are ambiguous on their own. Because `distance` is lower when a result is better,
`--score-order asc` keeps that direction when Jev's score is combined with it.

Fields such as `id` and `source` pass through unchanged. Field names have no built-in meaning;
the same command works with `content`, `summary`, `similarity`, or any other top-level names.

## Choose the Ranking Signal

With no score options, Jev's relevance probability decides the order:

```sh
search-command --json \
  | jev-reranker --query "authentication" --text-field body
```

When the input already has a useful distance or similarity score, pass `--score-field` and
`--score-order`. The default `boost` mode combines that score with Jev instead of throwing the
original signal away.

Use `--score-order asc` for distances where lower is better, and `--score-order desc` for
similarities where higher is better. `--weight` controls how strongly Jev affects the result.

<details>
<summary>Fusion formula</summary>

```text
# Lower is better
fusedScore = score / (1 + rerankScore * weight)

# Higher is better
fusedScore = score * (1 + rerankScore * weight)
```

Boost mode adds `fusedScore` and sorts in the selected direction. To ignore the source score
while leaving it in the output object, use `--fusion rerank-only`.

</details>

## JSON Contract

Stdin must contain one JSON array. Every array item must be an object with a string in the field
selected by `--text-field`, which defaults to `text`.

The CLI preserves unrecognized fields and writes `rerankScore` to every result. Boost mode also
writes `fusedScore`. Existing values under those output field names are replaced. Equal ranking
scores retain their input order, and `--top` is applied after sorting.

You may repeat `--context-field`. Present string values are prepended in flag order, while missing
and `null` values are skipped. In boost mode, every object must contain a finite number in the
selected score field.

An empty array returns `[]` without reading the API key or making a request.

## What Leaves Your Machine

The query, model name, selected context values, and selected text are sent directly to TypeSafe's
System One API. Other object fields, including the source score, stay local. The API key is read
only from `TYPESAFE_API_KEY`; there is no command-line key option.

Results are buffered until every batch succeeds, so a failed request leaves stdout empty instead
of producing a partial JSON document. Error messages do not include document text, response
bodies, request headers, or credentials.

## Options

| Option | Default | Description |
| --- | --- | --- |
| `--query <string>` | Required | Query used to judge relevance. |
| `--text-field <name>` | `text` | Object field containing the text to score. |
| `--context-field <name>` | None | Context field to prepend. May be repeated. |
| `--score-field <name>` | None | Existing numeric score to combine with Jev. |
| `--score-order <asc\|desc>` | None | Whether lower or higher source scores are better. Required with `--score-field`. |
| `--fusion <boost\|rerank-only>` | Automatic | Uses `boost` with a score field and `rerank-only` without one. |
| `--weight <number>` | `1.0` | Strength of the Jev boost. Valid only in boost mode. |
| `--top <n>` | All results | Number of results to keep after ranking. |
| `--model <name>` | `jev-latest` | Jev model route. |
| `--batch-size <n>` | `30` | Documents sent per request, from 1 through 30. |
| `--timeout-ms <n>` | `10000` | Timeout for each HTTP attempt, in milliseconds. |

## License

[MIT](LICENSE)

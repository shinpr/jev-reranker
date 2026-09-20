<p align="center">
  <img src="https://raw.githubusercontent.com/shinpr/jev-reranker/main/assets/banner.jpg" alt="Many candidate stones narrowing to one highlighted result" width="600" />
</p>

# Jev Reranker

[![CI](https://github.com/shinpr/jev-reranker/actions/workflows/ci.yml/badge.svg)](https://github.com/shinpr/jev-reranker/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/github/license/shinpr/jev-reranker)](LICENSE)

Choose the search results worth passing to your LLM.

`jev-reranker` uses TypeSafe AI's [Jev](https://docs.typesafe.ai/introduction) to rerank retrieved
documents, remove candidates that contain no usable evidence, or extract query-specific passages
for your LLM.

It reads a JSON array from stdin and writes a JSON array to stdout. Choose the field that contains
the text; IDs, source paths, retrieval scores, and other metadata pass through unchanged. Use it
after BM25, vector search, or any command that emits a JSON array of candidate objects.

```text
search or vector database -> JSON candidates -> Jev Reranker -> context for your LLM
```

## Install

Requires Node.js 14 or later on macOS, Linux, or Windows (x64 or Arm64).

Install the CLI from npm:

```sh
npm install --global jev-reranker
```

You can also run it without a global installation:

```sh
npx -y jev-reranker --help
```

Create an API key in the [TypeSafe dashboard](https://console.typesafe.ai/), then export it:

```sh
export TYPESAFE_API_KEY="your-api-key"
```

## Try It

Pipe an array of candidate documents into the CLI:

```sh
printf '%s\n' '[{"text":"Build artifacts are cached locally."},{"text":"Access tokens expire after one hour."}]' \
  | jev-reranker --query "How long do access tokens last?"
```

The CLI returns the same objects in best-first order, with a `rerankScore` from 0 to 1 added to
each one. Higher values mean Jev considers the result more relevant to the query.

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
      --top 5
```

`body` is the document text. `title` is prepended as context, which helps short chunks that are
ambiguous on their own. `distance`, `id`, and `source` pass through unchanged. Existing retrieval
scores do not affect Jev's judgment or the output order.

## Choose a Mode

| Mode | Use it to | Output |
| --- | --- | --- |
| `rerank` (default) | Put relevant candidates first. | Original objects sorted by `rerankScore`. |
| `filter` | Remove candidates that provide no usable evidence. | Retained objects in input order, with `evidenceScore`. |
| `compress` | Send shorter passages to your LLM. | Retained objects in input order, with `compressedText`. |

These modes run separately. To rank and then filter or compress, pipe one invocation into another
with the same query. Each invocation makes its own API requests.

<details>
<summary>API usage and retries</summary>

Rerank and filter make one sequential request per 30 candidates by default. Compression scores
every sentence or line, so long candidates can require more requests than ranking. Each compression
batch includes the full text and selected context of the documents it judges. A document with 100
units sends four copies of that context with `--batch-size 30`; smaller batches can increase the
total input sent to Jev and its cost. `--top` does not reduce API work or the number of extracted
units. See [TypeSafe's current Jev pricing](https://typesafe.ai/).

`--timeout-ms` applies to each HTTP attempt, not the whole run. HTTP 429 and 529 responses are
retried up to twice, after waits of 250 ms and 500 ms. Other HTTP errors, timeouts, and transport
failures are not retried. Total runtime depends on the number of batches and retries.

</details>

### Keep the useful evidence

A result can mention the right topic without providing an answer. Filter mode asks Jev whether
each candidate contains concrete evidence, including partial answers, conditions, and exceptions.

```sh
search-command --json \
  | jev-reranker --query "When can I request a refund?" --mode filter
```

Candidates with `evidenceScore >= 0.5` survive. Their input order stays intact, so you can keep
an upstream ranking you already trust. `--top 5` returns the first five survivors; it does not
sort them by evidence score. If none qualify, the result is `[]`.

### Extract shorter passages

```sh
search-command --json \
  | jev-reranker --query "When can I request a refund?" --mode compress
```

Compress mode splits the selected text into sentences and lines, then asks Jev which units to
keep. Jev sees the full source text and selected context when judging each unit, with instructions
to retain relevant conditions, exceptions, and references needed to understand the evidence.

`compressedText` contains retained passages in source order, with surrounding whitespace removed.
Adjacent retained text stays together; nonadjacent passages are separated by newlines.
The original text and metadata remain available. The example below shows the output shape:

```json
{
  "text": "Refunds are available within 30 days. Opened items are excluded. Our offices close at six.",
  "source": "/docs/refunds.md",
  "compressedText": "Refunds are available within 30 days. Opened items are excluded."
}
```

Pass `compressedText` to your downstream LLM to reduce its context. Passing the whole output
object also sends the original text and saves no space. Documents with no selected units are
omitted.

Sentence/line extraction is intended for prose. For code, tables, and unusual formatting,
whole-document `filter` mode may work better. Selected context fields help Jev interpret the text
but are not copied into `compressedText`.

### Adjust how much to keep

Both `filter` and `compress` accept `--threshold`, from 0 to 1, with a default of `0.5`.
Lower values keep more material; higher values discard more. In compress mode the threshold
applies to each sentence or line. Treat `0.5` as a starting point and tune it on your own data.

## JSON Contract

Stdin must contain one JSON array. Each item must be an object, and the field selected by
`--text-field` must be a string. The field defaults to `text`.

The CLI preserves the original fields and writes the selected mode's output field: `rerankScore`,
`evidenceScore`, or `compressedText`.

<details>
<summary>Full JSON contract</summary>

Existing values under the selected mode's output field are replaced. Other modes' output fields
pass through unchanged. Text and context fields cannot use the current mode's output field name.

Equal rerank scores retain their input order. Filter and compress preserve input order throughout.
`--top` limits output objects after the selected mode has processed all candidates.

You may repeat `--context-field`. Present string values are prepended in flag order, while missing
and `null` values are skipped. Other context value types are rejected.

An empty array returns `[]` without reading the API key or making a request. Compress also returns
`[]` without a request when every selected text is empty or contains only whitespace.

</details>

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
| `--mode <rerank\|filter\|compress>` | `rerank` | How to select or order context. |
| `--threshold <number>` | `0.5` | Minimum score to keep a document or unit. Filter and compress only. |
| `--top <n>` | All results | Maximum output objects, at least 1. |
| `--model <name>` | `jev-latest` | Jev model route. |
| `--batch-size <n>` | `30` | Judgments per request, from 1 through 30. Compress judges sentences/lines. |
| `--timeout-ms <n>` | `10000` | Timeout for each HTTP attempt, in milliseconds. |

## Background

[What Retrieval Still Hasn't Decided](https://www.norsica.jp/blog/what-retrieval-still-hasnt-decided)
covers why these three modes are separate judgments rather than stages of one pipeline, what each
one was measured against, and the deduplication mode that is not here.

## License

[MIT](LICENSE)

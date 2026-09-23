# Benchmarks

This report measures how much `jev-reranker` improves a BM25 ranking on three public BEIR
datasets. Reranking BM25's top 30 raised nDCG@10 by 0.06 to 0.13, and passing 100 candidates
instead of 30 helped further only on FiQA. It covers the default `rerank` mode of `jev-reranker`
0.1.4 with model `jev-1.13.0`, measured in September 2026, and does not compare other rerankers.

## Rerank Quality

nDCG@10 over each query's BM25 top 30 candidates, sent as one request per query. Runs 1 and 2
sent the measurement script's request body. The last column sent the exact body the CLI produces
(see Setup):

| Dataset | Queries | With a relevant candidate | BM25 order | Jev, run 1 | Jev, run 2 | Jev, CLI body |
| --- | --- | --- | --- | --- | --- | --- |
| SciFact (train) | 150 | 129 | 0.679 | 0.770 | 0.757 | 0.760 |
| NFCorpus (dev) | 133 | 106 | 0.266 | 0.328 | 0.328 | 0.331 |
| FiQA (test) | 150 | 87 | 0.236 | 0.363 | 0.359 | 0.366 |

Against BM25 order, run 1 gained +0.090 [95% CI +0.051, +0.132] on SciFact, +0.062
[+0.040, +0.085] on NFCorpus, and +0.127 [+0.092, +0.165] on FiQA.

Both runs sent identical requests, yet run 2 differed from run 1 by −0.013 on SciFact, −0.001
on NFCorpus, and −0.004 on FiQA. A document's score changed between the runs by 0.008 to 0.012
on average, depending on the dataset. That is enough to swap candidates whose scores were close.

## Candidate Depth

This comparison used an earlier, separate sample: 100 test queries per dataset (95 for
NFCorpus), none of them shared with the quality table. For each query, BM25's top 30 and top 100
were reranked, one run each. BM25 returned fewer than 100 candidates for 33 of the 95 NFCorpus
queries, so its "top 100" averaged 73 candidates:

| Dataset | Top 30 | Top 100 | Difference [95% CI] |
| --- | --- | --- | --- |
| SciFact | 0.765 | 0.762 | −0.003 [−0.023, +0.019] |
| NFCorpus | 0.415 | 0.423 | +0.009 [−0.008, +0.025] |
| FiQA | 0.357 | 0.403 | +0.047 [+0.012, +0.085] |

Only FiQA's interval excludes zero, and its +0.047 is more than three times the largest run-to-run
difference observed in the quality runs (0.013). Many of its relevant documents fall outside BM25's
top 30 but within the top 100: of the 262 judged relevant documents for these queries, the top 30
held 89 and the top 100 held 115, and Recall@10 rose from 0.355 to 0.427. SciFact's top 30 already
held 100 of its 117, and widening to 100 added three.

## Cost and Latency

Jev bills input tokens at $0.042 per million, and output tokens are free. On the depth
comparison's queries, 30 candidates averaged 9,500 input tokens per query on FiQA, 10,800 on
NFCorpus, and 13,500 on SciFact, or $0.0004 to $0.0006. With 100 candidates the averages were
32,200, 32,700, and 44,700 tokens, or $0.0014 to $0.0019. NFCorpus grows less than the others
because many of its queries have fewer than 100 candidates.

The latency sample is small: five FiQA queries, three runs each, from one machine. Measured as
the CLI's wall time including process startup, the median was 0.74 s for 30 candidates and
0.86 s for 100. Because the CLI sends batches concurrently, the four requests needed for 100
candidates added little wall time in this sample.

## What Did Not Help

Smaller batches did not improve nDCG@10. On the depth comparison's queries with 30 candidates,
one run each:

| `--batch-size` | SciFact | NFCorpus | FiQA | Cost |
| --- | --- | --- | --- | --- |
| 10 | −0.009 [−0.024, +0.004] | −0.008 [−0.021, +0.002] | +0.005 [+0.001, +0.011] | 5% more tokens |
| 1 | −0.015 [−0.039, +0.009] | −0.004 [−0.018, +0.011] | +0.000 [−0.018, +0.017] | 60 to 70% more tokens, 30 times the requests |

Differences are against the default of 30. FiQA's +0.005 at 10 is small and close to the
run-to-run variation seen in the quality runs. Its interval excludes zero, but the interval does
not include run-to-run variation. Every other interval includes zero. These results give no
reason to change the default `--batch-size` for rerank.

Mixing in the BM25 score did not help either. This used 60 SciFact and 60 NFCorpus test queries from
an earlier prompt comparison, with 20 candidates each and two runs averaged. A weighted sum with
min-max normalized BM25 scores, at weights 0.1, 0.2, 0.3, and 0.5, changed nDCG@10 by +0.002 and
−0.001 (SciFact and NFCorpus) at 0.1, and larger weights lowered it, reaching −0.041 and −0.015 at
0.5. Reciprocal rank fusion (k = 60) lowered it by 0.037 and 0.017. `jev-reranker` orders candidates
by Jev's score alone and keeps any retrieval score in the output object without using it.

## Setup

- **Model:** every request named `jev-1.13.0`. TypeSafe's model page listed it as the target of
  `jev-latest` on 2026-09-24.
- **Requests:** quality, depth, batch-size, and fusion runs were sent by scripts, not the CLI. Their
  saved requests use the rerank instruction text of `jev-reranker` 0.1.4. For one query per dataset,
  the body the CLI sends (captured from a local test endpoint) was compared with the body of the
  script behind the quality, depth, and batch-size runs. The model, the order of the documents, and
  each question's key and instruction matched. Two differences remain. When serializing, the CLI
  writes the question keys in string order (`document-0`, `document-1`, `document-10`, ...) where
  the script writes them in numeric order. And for a document with an empty title, as in FiQA, the
  CLI prepends two newlines to the text. Sending the CLI's exact body for all 433 quality queries
  changed nDCG@10 against run 1 by −0.010, +0.003, and +0.003, within the largest run-to-run
  difference (0.013), and changed document scores by 0.008 to 0.015 on average, about as much as a
  rerun. Latency runs used the 0.1.4 release binary.
- **Candidates:** a local Okapi BM25 (k1 = 1.2, b = 0.75, no stemming) over title and text. Its
  scores are not comparable with the official BEIR baselines.
- **Quality queries:** random samples that exclude every query used in earlier experiments.
  The SciFact and NFCorpus test splits had few such queries left, so these come from SciFact
  train and NFCorpus dev. Seventeen NFCorpus queries returned no BM25 candidates and are
  excluded.
- **Depth and batch-size queries:** sampled from the test splits before the quality runs.
  Five NFCorpus queries with no BM25 candidates are excluded.
- **Input:** the title was passed as context, the equivalent of `--context-field title`.
- **Metric:** nDCG@10 averaged over queries, with gain 2^rel − 1. The ideal ranking uses every
  judged document, so a relevant document outside the candidates lowers the score. Queries with
  no relevant candidate stay in every average.
- **Intervals:** 95% paired bootstrap over queries, 10,000 resamples. They do not include
  run-to-run variation.
- **Failures:** every request succeeded after retries. The scripts retried HTTP 429 and 529
  responses; the number of retries was not recorded.

The measurement scripts and query IDs are not published.

## Limits

The three datasets are English, and each labels relevance for its own task: abstracts that
support or refute a scientific claim, medical articles for a nutrition question, answer posts for a
financial question. Results on your corpus and queries can differ.

NFCorpus has graded labels, so its values differ from BEIR's pytrec_eval, which uses the label
itself as the gain. SciFact and FiQA labels are binary, and the two definitions agree there.

Filter and compress are not benchmarked here. Their evaluations so far use small, hand-built
label sets that do not support a published accuracy figure.

When `jev-latest` moves past `jev-1.13.0`, these numbers may change.

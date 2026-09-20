use std::collections::BTreeMap;
use std::env;
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::Value;

use crate::error::AppError;
use crate::options::ResolvedOptions;
use crate::preparation::PreparedDocument;

const PRODUCTION_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const TEST_ENDPOINT_VARIABLE: &str = "JEV_RERANKER_TEST_ENDPOINT";
const API_KEY_VARIABLE: &str = "TYPESAFE_API_KEY";

#[derive(Serialize)]
struct SystemOneRequest<'a> {
    state: RequestState<'a>,
    model: &'a str,
    questions: BTreeMap<String, NoulQuestion>,
}

#[derive(Serialize)]
struct RequestState<'a> {
    query: &'a str,
    documents: Vec<RequestDocument<'a>>,
}

#[derive(Serialize)]
struct RequestDocument<'a> {
    id: String,
    text: &'a str,
}

#[derive(Serialize)]
struct NoulQuestion {
    #[serde(rename = "type")]
    question_type: &'static str,
    instructions: String,
}

pub fn score_documents(
    documents: &[PreparedDocument],
    options: &ResolvedOptions,
) -> Result<Vec<f64>, AppError> {
    if documents.is_empty() {
        return Ok(Vec::new());
    }

    let endpoint = endpoint()?;
    let api_key = api_key()?;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(options.timeout_ms))
        .build()
        .map_err(|_| AppError::HttpClient)?;
    let context = RequestContext {
        client: &client,
        endpoint: &endpoint,
        api_key: &api_key,
        options,
    };
    let mut scores = Vec::with_capacity(documents.len());
    for (batch_index, batch) in documents.chunks(options.batch_size).enumerate() {
        let batch_scores = request_batch(&context, batch_index + 1, batch)?;
        scores.extend(batch_scores);
    }
    Ok(scores)
}

struct RequestContext<'a> {
    client: &'a Client,
    endpoint: &'a str,
    api_key: &'a str,
    options: &'a ResolvedOptions,
}

fn endpoint() -> Result<String, AppError> {
    #[cfg(debug_assertions)]
    if let Some(value) = env::var_os(TEST_ENDPOINT_VARIABLE) {
        let value = value.to_str().ok_or(AppError::InvalidTestEndpoint)?;
        let mut url = reqwest::Url::parse(value).map_err(|_| AppError::InvalidTestEndpoint)?;
        let loopback = url.host_str() == Some("127.0.0.1");
        if url.scheme() != "http"
            || !loopback
            || url.username() != ""
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(AppError::InvalidTestEndpoint);
        }
        if url.path().is_empty() || url.path() == "/" {
            url.set_path("/v1/systemone");
        }
        return Ok(url.to_string());
    }

    #[cfg(not(debug_assertions))]
    if env::var_os(TEST_ENDPOINT_VARIABLE).is_some() {
        return Err(AppError::ReleaseTestEndpoint);
    }

    Ok(PRODUCTION_ENDPOINT.to_owned())
}

fn api_key() -> Result<String, AppError> {
    match env::var(API_KEY_VARIABLE) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(AppError::MissingCredential),
    }
}

fn request_batch(
    context: &RequestContext<'_>,
    batch: usize,
    documents: &[PreparedDocument],
) -> Result<Vec<f64>, AppError> {
    let request = compose_request(documents, context.options);
    let ids = documents
        .iter()
        .map(|document| document_id(document.original_index))
        .collect::<Vec<_>>();
    for attempt in 0..=2 {
        let result = context
            .client
            .post(context.endpoint)
            .bearer_auth(context.api_key)
            .json(&request)
            .send();
        let response = match result {
            Ok(response) => response,
            Err(error) if error.is_timeout() => {
                return Err(AppError::HttpTimeout { batch });
            }
            Err(_) => {
                return Err(AppError::HttpTransport { batch });
            }
        };
        let status = response.status().as_u16();
        if status == 429 || status == 529 {
            if attempt < 2 {
                let delay = if attempt == 0 { 250 } else { 500 };
                thread::sleep(Duration::from_millis(delay));
                continue;
            }
            return Err(AppError::HttpRetryExhausted { batch, status });
        }
        if !response.status().is_success() {
            return Err(AppError::HttpStatus { batch, status });
        }
        let body = response
            .json::<Value>()
            .map_err(|_| AppError::InvalidResponse { batch })?;
        return parse_answers(&body, &ids, batch);
    }
    Err(AppError::HttpRetryExhausted { batch, status: 529 })
}

fn compose_request<'a>(
    documents: &'a [PreparedDocument],
    options: &'a ResolvedOptions,
) -> SystemOneRequest<'a> {
    let mut request_documents = Vec::with_capacity(documents.len());
    let mut questions = BTreeMap::new();
    for document in documents {
        let id = document_id(document.original_index);
        request_documents.push(RequestDocument {
            id: id.clone(),
            text: &document.prepared_text,
        });
        questions.insert(
            id.clone(),
            NoulQuestion {
                question_type: "noul",
                instructions: format!(
                    "Is the document whose id is \"{id}\" relevant to the query in state.query? Use only that document's text and the query."
                ),
            },
        );
    }
    SystemOneRequest {
        state: RequestState {
            query: &options.query,
            documents: request_documents,
        },
        model: &options.model,
        questions,
    }
}

fn document_id(index: usize) -> String {
    format!("document-{index}")
}

fn parse_answers(body: &Value, ids: &[String], batch: usize) -> Result<Vec<f64>, AppError> {
    let Some(root) = body.as_object() else {
        return Err(AppError::InvalidResponse { batch });
    };
    let Some(answers) = root.get("answers").and_then(Value::as_object) else {
        return Err(AppError::InvalidResponse { batch });
    };
    if answers.len() != ids.len()
        || answers
            .keys()
            .any(|answer_id| !ids.iter().any(|id| id == answer_id))
    {
        return Err(AppError::InvalidResponse { batch });
    }

    let mut scores = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(answer) = answers.get(id).and_then(Value::as_object) else {
            return Err(AppError::InvalidResponse { batch });
        };
        if answer.get("type") != Some(&Value::String("noul".to_owned())) {
            return Err(AppError::InvalidResponse { batch });
        }
        let Some(score) = answer.get("noul").and_then(Value::as_f64) else {
            return Err(AppError::InvalidResponse { batch });
        };
        if !score.is_finite() || !(0.0..=1.0).contains(&score) {
            return Err(AppError::InvalidResponse { batch });
        }
        scores.push(score);
    }
    Ok(scores)
}

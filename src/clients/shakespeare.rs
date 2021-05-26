use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Url};
use serde::{Serialize, Deserialize};
use tracing::{instrument, debug};

use crate::metrics;

/// A `ShakespeareString` represents a string converted to Shakespearean language.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShakespeareString(String);

impl ShakespeareString {

  /// Returns a reference to the inner string owned by this `ShakespeareString`.
  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// Consumes this `ShakespeareString` and returns the inner string.
  pub fn into_str(self) -> String {
    self.0
  }

}

/// A client for the Shakespeare Translator API.
#[derive(Clone)]
pub struct ShakespeareClient {
  client: Client,
  endpoint_url: String
}

/// The response from the Shakespeare Translator API.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ShakespeareTranslatorResponse {
  Error {
    error: ShakespeareTranslatorError
  },
  Success {
    contents: ShakespeareTranslatorContents
  }
}

#[derive(Serialize, Deserialize)]
struct ShakespeareTranslatorError {
  code: u16,
  message: String
}

#[derive(Serialize, Deserialize)]
struct ShakespeareTranslatorContents {
  translated: String,
  text: String
}

impl ShakespeareClient {

  /// Creates a new [`ShakespeareClient`](crate::clients::ShakespeareClient) using the given base url.
  /// 
  /// The requests will be performed against `<base_url>/translate/shakespeare.json`.
  pub fn new(base_url: &str) -> Result<Self> {
    Ok(ShakespeareClient {
      client: Client::new(),
      endpoint_url:
        Url::parse(base_url)
          .context("Invalid Shakespeare Translator base URL")?
          .join("translate/shakespeare.json")?
          .into()
    })
  }

  /// Requests the translation to Shakespearean language of the given string.
  #[instrument(skip(self), err)]
  pub async fn translate(&self, text: &str) -> Result<ShakespeareString> {

    debug!("Sending HTTP request");
    metrics::SHAKESPEARE_REQUESTS.inc();

    let mut params = HashMap::new();
    params.insert("text", text);

    // Send the request
    let res = self.client.post(&self.endpoint_url)
      .form(&params)
      .send()
      .await
      .context("Cannot send request to Shakespeare Translator")?;

    debug!(status = res.status().as_u16(), "Got HTTP response: {}", res.status().as_u16());

    // Handle error statuses
    if res.status().is_server_error() {
      return Err(anyhow!("HTTP error: {}", res.status().as_u16()));
    }

    // Parse the body of the response
    let body = res
      .json::<ShakespeareTranslatorResponse>()
      .await
      .context("Cannot parse response from Shakespeare Translator")?;

    // Check if the server returned an error
    return match body {
      ShakespeareTranslatorResponse::Error { error } => {
        Err(anyhow!("Shakespeare Translator error: {}", &error.message))
      },
      ShakespeareTranslatorResponse::Success { contents } => {
        Ok(ShakespeareString(contents.translated))
      }
    }

  }

}

#[cfg(test)]
mod test {
  use super::*;
  use httpmock::{MockServer, Method};

  async fn mock_response(text: &str, res: ShakespeareTranslatorResponse) -> Result<ShakespeareString> {
    
    // Prepare a server with a mock response
    let server = MockServer::start_async().await;
    let mock = server.mock_async(|when, then| {
      when.method(Method::POST)
        .path("/translate/shakespeare.json")
        .body(
          form_urlencoded::Serializer::new(String::new())
            .append_pair("text", text)
            .finish()
        );

      let status = if let ShakespeareTranslatorResponse::Error { error } = &res {
        error.code
      } else {
        200
      };
      then.status(status).json_body_obj(&res);
    }).await;

    // Build a new client and perform the request
    let client = ShakespeareClient::new(&server.base_url()).unwrap();
    let res = client.translate(text).await;

    // Assert that the mock matched
    mock.assert();

    // Return the response from the client
    res

  }

  async fn mock_server_error_response(text: &str) -> Result<ShakespeareString> {
    
    // Prepare a server with a mock response
    let server = MockServer::start_async().await;
    let mock = server.mock_async(|when, then| {
      when.method(Method::POST)
        .path("/translate/shakespeare.json")
        .body(
          form_urlencoded::Serializer::new(String::new())
            .append_pair("text", text)
            .finish()
        );
      then.status(500)
        .body("Internal server error");
    }).await;

    // Build a new client and perform the request
    let client = ShakespeareClient::new(&server.base_url()).unwrap();
    let res = client.translate(text).await;

    // Assert that the mock matched
    mock.assert();

    // Return the response from the client
    res

  }

  #[tokio::test]
  async fn test_successful_response() {

    let translated = mock_response("Hello world", ShakespeareTranslatorResponse::Success {
      contents: ShakespeareTranslatorContents {
        translated: "Mocked translation".to_string(),
        text: "Hello world".to_string()
      }
    }).await;

    assert_eq!(translated.unwrap().as_str(), "Mocked translation");

  }

  #[tokio::test]
  async fn test_error_response() {

    let translated = mock_response("Hello world", ShakespeareTranslatorResponse::Error {
      error: ShakespeareTranslatorError {
        code: 429,
        message: "Mocked error".to_string()
      }
    }).await;

    assert!(translated.is_err());
    assert!(translated.unwrap_err().to_string().contains("Mocked error"));

  }

  #[tokio::test]
  async fn test_server_error_response() {
    
    let translated = mock_server_error_response("Hello world").await;

    assert!(translated.is_err());
    assert!(translated.unwrap_err().to_string().contains("HTTP error: 500"));

  }
}
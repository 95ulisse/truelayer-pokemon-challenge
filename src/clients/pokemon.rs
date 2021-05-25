use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use lru::LruCache;
use reqwest::{Client, Url};
use serde::{Serialize, Deserialize};
use tracing::{instrument, debug};

/// A client for the Pokemon APIs.
#[derive(Clone)]
pub struct PokemonClient {
  client: Client,
  endpoint_url: Url,
  cache: Arc<Mutex<LruCache<String, String>>>
}

/// The response from the Pokemon API.
#[derive(Serialize, Deserialize)]
struct PokemonSpecies {
  flavor_text_entries: Vec<PokemonFlavorTextEntry>
}

#[derive(Serialize, Deserialize)]
struct PokemonFlavorTextEntry {
  flavor_text: String,
  language: PokemonLanguage
}

#[derive(Serialize, Deserialize)]
struct PokemonLanguage {
  name: String
}

impl PokemonClient {

  /// Creates a new [`PokemonClient`](crate::clients::PokemonClient) using the given base url.
  pub fn new(base_url: &str, cache_size: usize) -> Result<Self> {
    Ok(PokemonClient {
      client: Client::new(),
      endpoint_url: Url::parse(base_url).context("Invalid Pokemon API base URL")?,
      cache: Arc::new(Mutex::new(LruCache::new(cache_size)))
    })
  }

  /// Retrieves the description of the Pokemon with the given name.
  /// If no Pokemon can be found, `None` is returned.
  #[instrument(skip(self), err)]
  pub async fn get_pokemon_description(&self, name: &str) -> Result<Option<String>> {

    let name = name.to_lowercase();

    // Before sending the request, check if we have a cached description
    if let Some(cached) = self.cache.lock().unwrap().get(&name) {
      debug!("Cache hit");
      return Ok(Some(cached.to_string()));
    }

    debug!("Sending HTTP request");

    // Send the request
    let res = self.client.get(self.endpoint_url.join("pokemon-species/")?.join(&name)?)
      .send()
      .await
      .context("Cannot send request to Pokemon API")?;

    debug!(status = res.status().as_u16(), "Got HTTP response: {}", res.status().as_u16());

    // If the pokemon has not been found, exit immediately
    if res.status().as_u16() == 404 {
      return Ok(None);
    } else if res.status().is_server_error() {
      return Err(anyhow!("HTTP error: {}", res.status().as_u16()));
    }

    // Parse the body of the response
    let body = res
      .json::<PokemonSpecies>()
      .await
      .context("Cannot parse response from Pokemon API")?;

    // Select the first english description available
    let description = body.flavor_text_entries
      .into_iter()
      .find(|entry| entry.language.name == "en")
      .map(|entry| entry.flavor_text);

    // Cache the extracted description
    if let Some(x) = &description {
      self.cache.lock().unwrap().put(name.to_string(), x.to_string());
    }

    description
      .map(Some)
      .ok_or_else(|| anyhow!("No english description is available"))

  }

}

#[cfg(test)]
mod test {
  use super::*;
  use httpmock::{MockServer, Method};
  use regex::Regex;

  async fn mock_description_response(name: &str, entries: Vec<PokemonFlavorTextEntry>) -> Result<Option<String>> {
    
    // Prepare a server with a mock response
    let server = MockServer::start_async().await;
    let mock = server.mock_async(|when, then| {
      when.method(Method::GET)
        .path(format!("/pokemon-species/{}", name));
      then.status(200)
        .json_body_obj(&PokemonSpecies {
          flavor_text_entries: entries
        });
    }).await;

    // Build a new client and perform the request
    let client = PokemonClient::new(&server.base_url(), 0).unwrap();
    let res = client.get_pokemon_description(name).await;

    // Assert that the mock matched
    mock.assert();

    // Return the response from the client
    res

  }

  async fn mock_server_error_response(name: &str) -> Result<Option<String>> {
    
    // Prepare a server with a mock response
    let server = MockServer::start_async().await;
    let mock = server.mock_async(|when, then| {
      when.method(Method::GET)
        .path(format!("/pokemon-species/{}", name));
      then.status(500)
        .body("Internal server error");
    }).await;

    // Build a new client and perform the request
    let client = PokemonClient::new(&server.base_url(), 0).unwrap();
    let res = client.get_pokemon_description(name).await;

    // Assert that the mock matched
    mock.assert();

    // Return the response from the client
    res

  }

  async fn mock_not_found_response(name: &str) -> Result<Option<String>> {
    
    // Prepare a server with a mock response
    let server = MockServer::start_async().await;
    let mock = server.mock_async(|when, then| {
      when.method(Method::GET)
        .path(format!("/pokemon-species/{}", name));
      then.status(404)
        .body("Not found");
    }).await;

    // Build a new client and perform the request
    let client = PokemonClient::new(&server.base_url(), 0).unwrap();
    let res = client.get_pokemon_description(name).await;

    // Assert that the mock matched
    mock.assert();

    // Return the response from the client
    res

  }

  #[tokio::test]
  async fn test_single_english_description() {

    let res = mock_description_response("pikachu", vec![
      PokemonFlavorTextEntry {
        flavor_text: "This one!".to_string(),
        language: PokemonLanguage {
          name: "en".to_string()
        }
      }
    ]).await;

    assert_eq!(res.unwrap(), Some("This one!".to_string()));

  }

  #[tokio::test]
  async fn test_multiple_english_descriptions() {

    let res = mock_description_response("pikachu", vec![
      PokemonFlavorTextEntry {
        flavor_text: "This one!".to_string(),
        language: PokemonLanguage {
          name: "en".to_string()
        }
      },
      PokemonFlavorTextEntry {
        flavor_text: "Not this one".to_string(),
        language: PokemonLanguage {
          name: "en".to_string()
        }
      }
    ]).await;

    assert_eq!(res.unwrap(), Some("This one!".to_string()));
    
  }

  #[tokio::test]
  async fn test_no_english_description() {
    
    let res = mock_description_response("pikachu", vec![
      PokemonFlavorTextEntry {
        flavor_text: "Non questa qui".to_string(),
        language: PokemonLanguage {
          name: "it".to_string()
        }
      }
    ]).await;

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("No english description is available"));

  }

  #[tokio::test]
  async fn test_no_description() {

    let res = mock_description_response("pikachu", vec![]).await;

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("No english description is available"));

  }

  #[tokio::test]
  async fn test_pokemon_not_found() {
    
    let res = mock_not_found_response("pikachu").await;

    assert!(res.unwrap().is_none());

  }

  #[tokio::test]
  async fn test_server_error() {
    
    let res = mock_server_error_response("pikachu").await;

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("HTTP error: 500"));

  }

  #[tokio::test]
  async fn test_caching_behaviour() {

    // Prepare a server serving always the same response
    let server = MockServer::start_async().await;
    let mock = server.mock_async(|when, then| {
      when.method(Method::GET)
        .path_matches(Regex::new("^/pokemon-species/").unwrap());
      then.status(200)
        .json_body_obj(&PokemonSpecies {
          flavor_text_entries: vec![
            PokemonFlavorTextEntry {
              flavor_text: "This one!".to_string(),
              language: PokemonLanguage {
                name: "en".to_string()
              }
            }
          ]
        });
    }).await;

    // Build a new client with a cache of 1 item and perform the first request.
    // The first request will go through, since its the first one.
    let client = PokemonClient::new(&server.base_url(), 1).unwrap();
    assert_eq!(client.get_pokemon_description("pikachu").await.unwrap(), Some("This one!".to_string()));
    mock.assert_hits(1);

    // Now perform the same request and assert that the backend APIs have not been contacted a second time
    assert_eq!(client.get_pokemon_description("pikachu").await.unwrap(), Some("This one!".to_string()));
    mock.assert_hits(1);

    // Ask for the description of another pokemon
    assert_eq!(client.get_pokemon_description("bulbasaur").await.unwrap(), Some("This one!".to_string()));
    mock.assert_hits(2);

    // Now the second pokemon is cached
    assert_eq!(client.get_pokemon_description("bulbasaur").await.unwrap(), Some("This one!".to_string()));
    mock.assert_hits(2);

    // And if we ask for the first one, another request is fired bacause the cache is for only one item
    assert_eq!(client.get_pokemon_description("pikachu").await.unwrap(), Some("This one!".to_string()));
    mock.assert_hits(3);

  }

}
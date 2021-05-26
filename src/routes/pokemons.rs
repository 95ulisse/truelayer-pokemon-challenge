use serde::Serialize;
use tracing::debug;
use warp::Rejection;

use crate::routes::State;
use crate::routes::errors::CustomRejection;

#[derive(Serialize)]
pub struct GetPokemonReponse {
  name: String,
  description: String
}

/// Handler for the `GET /pokemon/{name}` route.
pub async fn handle_get_pokemon(pokemon_name: String, state: State) -> std::result::Result<GetPokemonReponse, Rejection> {

  // Before sending the request, check if we have a cached description
  if let Some(cached) = state.cache.lock().unwrap().get(&pokemon_name) {
    debug!("Cache hit");
    return Ok(GetPokemonReponse {
      name: pokemon_name,
      description: cached.clone()
    });
  }

  // First step: get the description of the pokemon
  let description = state.pokemon_client.get_pokemon_description(&pokemon_name).await
    .map_err(CustomRejection::new)?;

  match description {
    None => {

      // Return a 404 if no pokemon has been found
      Err(warp::reject::not_found())

    },
    Some(description) => {

      // Translate the description and compose the final reply
      let translated = state.shakespeare_client.translate(&description).await
        .map_err(CustomRejection::new)?;

      // Cache the computed result
      state.cache.lock().unwrap().put(pokemon_name.clone(), translated.as_str().to_string());

      Ok(GetPokemonReponse {
        name: pokemon_name,
        description: translated.into_str()
      })
      
    }
  }

}

#[cfg(test)]
mod test {
  use super::*;
  use crate::clients::{PokemonClient, ShakespeareClient};
  use std::sync::{Arc, Mutex};
  use httpmock::{MockServer, Method};
  use lru::LruCache;
  use regex::Regex;
  use serde_json::json;

  #[tokio::test]
  async fn test_caching_behaviour() {

    // Prepare a mock for both the Pokemon and the Shakespeare API
    // - Pokemon API returns description "This one!"
    // - Shakespeare API translates "This one!" into "Mocked translation"
    let server = MockServer::start_async().await;
    let pokemon_mock = server.mock_async(|when, then| {
      when.method(Method::GET)
        .path_matches(Regex::new("^/pokemon-species/").unwrap());
      then.status(200)
        .json_body(json!({
          "flavor_text_entries": [
            {
              "flavor_text": "This one!",
              "language": {
                "name": "en"
              }
            }
          ]
        }));
    }).await;
    let shakespeare_mock = server.mock_async(|when, then| {
      when.method(Method::POST)
        .path("/translate/shakespeare.json")
        .body(
          form_urlencoded::Serializer::new(String::new())
            .append_pair("text", "This one!")
            .finish()
        );
      then.status(200)
        .json_body(json!({
          "contents": {
            "translated": "Mocked translation",
            "text": "This one!"
          }
        }));
    }).await;

    // Build the app state
    let state = State {
      pokemon_client: PokemonClient::new(&server.base_url()).unwrap(),
      shakespeare_client: ShakespeareClient::new(&server.base_url()).unwrap(),
      cache: Arc::new(Mutex::new(LruCache::new(1)))
    };

    // Perform the first request.
    // The first request will go through, since its the first one.
    assert_eq!(handle_get_pokemon("pikachu".to_string(), state.clone()).await.unwrap().description, "Mocked translation");
    pokemon_mock.assert_hits(1);
    shakespeare_mock.assert_hits(1);

    // Now perform the same request and assert that the backend APIs have not been contacted a second time
    assert_eq!(handle_get_pokemon("pikachu".to_string(), state.clone()).await.unwrap().description, "Mocked translation");
    pokemon_mock.assert_hits(1);
    shakespeare_mock.assert_hits(1);

    // Ask for the description of another pokemon
    assert_eq!(handle_get_pokemon("bulbasaur".to_string(), state.clone()).await.unwrap().description, "Mocked translation");
    pokemon_mock.assert_hits(2);
    shakespeare_mock.assert_hits(2);

    // Now the second pokemon is cached
    assert_eq!(handle_get_pokemon("bulbasaur".to_string(), state.clone()).await.unwrap().description, "Mocked translation");
    pokemon_mock.assert_hits(2);
    shakespeare_mock.assert_hits(2);

    // And if we ask for the first one, another request is fired bacause the cache is for only one item
    assert_eq!(handle_get_pokemon("pikachu".to_string(), state.clone()).await.unwrap().description, "Mocked translation");
    pokemon_mock.assert_hits(3);
    shakespeare_mock.assert_hits(3);

  }
}
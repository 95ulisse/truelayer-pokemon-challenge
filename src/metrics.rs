use lazy_static::lazy_static;
use prometheus::{IntCounter, register_int_counter};

lazy_static! {
  
  pub static ref POKEAPI_REQUESTS: IntCounter =
    register_int_counter!("pokechallenge_pokeapi_requests", "Requests to the PokeAPI service").unwrap();

  pub static ref SHAKESPEARE_REQUESTS: IntCounter =
    register_int_counter!("pokechallenge_shakespeare_requests", "Requests to the Shakespeare Translator service").unwrap();

  pub static ref CACHE_HITS: IntCounter =
    register_int_counter!("pokechallenge_cache_hits", "Number of cache hits").unwrap();

}
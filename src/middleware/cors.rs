use actix_cors::Cors;
use actix_web::http::header::{self};
use log::warn;
use std::collections::HashSet;

pub fn setup_cors(cors_domains: &Vec<String>) -> Cors {
    let allowed_domains_set: HashSet<String> = cors_domains.iter().cloned().collect();

    Cors::default()
        .allowed_origin_fn(move |origin, _req_head| {
            if let Ok(origin_str) = origin.to_str() {
                allowed_domains_set.contains(origin_str)
            } else {
                warn!("CORS blocked: {:?}", origin);
                false
            }
        })
        .allowed_methods(vec!["GET", "POST"])
        .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
        .allowed_header(header::CONTENT_TYPE)
        .max_age(3600)
}

use actix_cors::Cors;
use actix_web::http::header::{self};
use log::{warn, info, debug};
use std::collections::HashSet;

pub fn setup_cors(cors_domains: &Vec<String>) -> Cors {
    info!("🔒 Setting up CORS middleware");
    debug!("🔒 Allowed CORS domains: {:?}", cors_domains);
    
    let allowed_domains_set: HashSet<String> = cors_domains.iter().cloned().collect();

    Cors::default()
        .allowed_origin_fn(move |origin, _req_head| match origin.to_str() {
            Ok(origin_str) => {
                let is_allowed = allowed_domains_set.contains(origin_str);
                if is_allowed {
                    debug!("✅ CORS allowed - Origin: {}", origin_str);
                } else {
                    warn!("🚫 CORS blocked - Origin: {} (not in allowed list: {:?})", origin_str, allowed_domains_set);
                }
                is_allowed
            }
            Err(e) => {
                warn!("🚫 CORS blocked - Invalid origin header: {}", e);
                false
            }
        })
        .allowed_methods(vec!["GET", "POST"])
        .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
        .allowed_header(header::CONTENT_TYPE)
        .max_age(3600)
}

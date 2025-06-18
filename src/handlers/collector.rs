use crate::config::Config;
use crate::db::DbPool;
use crate::models::Collector;
use crate::utils::geoip::geoip_lookup;
use actix_web::error::BlockingError;
use actix_web::{http, web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error;
use log::{info, warn, error, debug};
use std::sync::Arc;
use ulid::Ulid;
use woothee::parser::Parser;

fn generate_analytics_js(cid: &str, app_url: &str) -> String {
    debug!("📝 Generating analytics JS for collector: {}", cid);
    format!(
        r#""use strict";
(function() {{
    var collectorId = "{}";
    var appUrl = "{}";

    function init() {{
        document.addEventListener('click', function(event) {{
            if (event.target.tagName === 'A') {{
                var target = event.target.getAttribute('target');
                var href = event.target.getAttribute('href');
                
                if (target === '_blank') {{
                    stats_collect('leave', href);
                }}
            }}
        }});

        window.addEventListener("beforeunload", function(event) {{
            stats_collect('exit');
        }});

        // Listen for history changes
        function wrapHistoryMethod(method) {{
            var original = history[method];
            history[method] = function(state, title, url) {{
                var fullUrl = new URL(url, window.location.origin).href;
                console.log("📼 history", method, url, fullUrl);
                original.apply(this, arguments);
                stats_collect('visit', fullUrl);
            }};
        }}
    
        wrapHistoryMethod('pushState');
        wrapHistoryMethod('replaceState');
    
        // Listen for popstate event
        window.addEventListener('popstate', function(event) {{
            stats_collect('visit', location.href);
        }});
    }}

    async function send(type = "pageview", url_override = null, referrer = document.referrer) {{
        var url = new URL(appUrl + "/collect");

        url.searchParams.set('collector_id', collectorId);
        url.searchParams.set('name', type);
        url.searchParams.set('url', url_override || window.location.href);
        url.searchParams.set('referrer', referrer);

        fetch(url)
        .then(res => res.json())
        .then(data => {{
            // console.log("📼", data);
        }})
        .catch(rejected => {{
            console.log("📼", "failed to collect");
        }});
    }}

    async function stats_collect(type, url = null) {{
        await send(type, url);
    }}

    window.stats_collect = stats_collect;
    stats_collect('enter');

    window.addEventListener('load', function() {{
        init();
    }});
}})();
"#,
        cid, app_url
    )
}

fn create_collector(
    pool: &web::Data<DbPool>,
    origin_str: &str,
    lookup_country: &str,
    lookup_city: &str,
    os_option: Option<String>,
    browser_option: Option<String>,
) -> Result<String, Error> {
    debug!(
        "🏗️ Creating collector - Origin: {}, Country: {}, City: {}, OS: {:?}, Browser: {:?}",
        origin_str, lookup_country, lookup_city, os_option, browser_option
    );

    use crate::schema::collectors::dsl::*;

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for collector creation");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for collector creation: {}", e);
            return Err(Error::RollbackTransaction);
        }
    };

    let collector_id = Ulid::new().to_string();
    let new_collector = Collector {
        id: collector_id.clone(),
        origin: origin_str.to_string(),
        country: lookup_country.to_string(),
        city: lookup_city.to_string(),
        os: os_option.clone(),
        browser: browser_option.clone(),
        timestamp: Utc::now().naive_utc(),
    };

    match diesel::insert_into(collectors)
        .values(&new_collector)
        .execute(&mut conn)
    {
        Ok(_) => {
            info!(
                "✅ Collector created successfully - ID: {}, Origin: {}, Country: {}, City: {}",
                collector_id, origin_str, lookup_country, lookup_city
            );
            Ok(collector_id)
        }
        Err(e) => {
            error!(
                "❌ Failed to insert collector - Origin: {}, Error: {}",
                origin_str, e
            );
            Err(e)
        }
    }
}

async fn create_collector_from_request(
    req: HttpRequest,
    pool: web::Data<DbPool>,
) -> Result<Result<String, Error>, BlockingError> {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let origin = req.headers().get("Origin").map_or_else(
        || {
            warn!("⚠️ No Origin header found - IP: {}", client_ip);
            "unknown".to_owned()
        },
        |v| v.to_str().unwrap_or("unknown").to_owned(),
    );

    info!(
        "🔍 Collector creation request - IP: {}, Origin: {}",
        client_ip, origin
    );

    let db_path = "data/GeoLite2-City.mmdb";
    let real_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok());
    let ip: &str = real_ip.unwrap_or("0.0.0.0");

    debug!("🌍 Using IP for GeoIP lookup: {}", ip);

    let mut os: Option<String> = None;
    let mut browser: Option<String> = None;

    if let Some(user_agent_string) = req.headers().get("User-Agent") {
        if let Ok(ua_string) = user_agent_string.to_str() {
            debug!("🔍 Parsing User-Agent: {}", ua_string);
            let parser = Parser::new();
            let result = parser.parse(ua_string);
            if let Some(ref parsed_result) = result {
                os = Some(parsed_result.os.to_string());
                browser = Some(parsed_result.name.to_string());
                debug!(
                    "🖥️ Parsed User-Agent - OS: {:?}, Browser: {:?}",
                    os, browser
                );
            } else {
                warn!("⚠️ Failed to parse User-Agent: {}", ua_string);
            }
        }
    } else {
        warn!("⚠️ No User-Agent header found - IP: {}", client_ip);
    }

    let (lookup_country, lookup_city) = match geoip_lookup(ip, db_path) {
        Ok((country, city)) => {
            debug!("🌍 GeoIP lookup successful - IP: {}, Country: {}, City: {}", ip, country, city);
            (country, city)
        }
        Err(e) => {
            warn!("⚠️ GeoIP lookup failed - IP: {}, Error: {}", ip, e);
            ("Unknown".to_owned(), "Unknown".to_owned())
        }
    };

    web::block(move || {
        create_collector(
            &pool,
            &origin,
            &lookup_country,
            &lookup_city,
            os.clone(),
            browser.clone(),
        )
    })
    .await
}

pub async fn serve_collector_js(
    req: HttpRequest,
    config: web::Data<Arc<Config>>,
    pool: web::Data<DbPool>,
) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📜 Analytics JS request - IP: {}", client_ip);

    let collector_result = create_collector_from_request(req, pool).await;

    match collector_result {
        Ok(collector_id) => match collector_id {
            Ok(id) => {
                debug!("📜 Generating analytics JS for collector: {}", id);
                let js_content = generate_analytics_js(&id, &config.app_url);
                info!("✅ Analytics JS served successfully - Collector: {}, IP: {}", id, client_ip);
                HttpResponse::Ok()
                    .insert_header((http::header::CACHE_CONTROL, "public, max-age=1800")) // cache for 30 minutes
                    .content_type("application/javascript")
                    .body(js_content)
            }
            Err(e) => {
                error!("❌ Error creating collector for JS request - IP: {}, Error: {}", client_ip, e);
                HttpResponse::InternalServerError().json("Failed to create collector")
            }
        },
        Err(e) => {
            error!("❌ Error serving collector JS - IP: {}, Error: {}", client_ip, e);
            HttpResponse::InternalServerError().json("Failed to serve analytics JS")
        }
    }
}

pub async fn post_collector(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📝 POST collector request - IP: {}", client_ip);

    let collector_result = create_collector_from_request(req, pool).await;

    match collector_result {
        Ok(collector_id) => match collector_id {
            Ok(id) => {
                info!("✅ Collector created via POST - ID: {}, IP: {}", id, client_ip);
                HttpResponse::Ok().json(id)
            }
            Err(e) => {
                error!("❌ Error creating collector via POST - IP: {}, Error: {}", client_ip, e);
                HttpResponse::InternalServerError().json("Failed to create collector")
            }
        },
        Err(e) => {
            error!("❌ Error processing POST collector request - IP: {}, Error: {}", client_ip, e);
            HttpResponse::InternalServerError().json("Failed to process collector request")
        }
    }
}

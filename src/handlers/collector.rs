use crate::config::Config;
use crate::db::DbPool;
use crate::models::Collector;
use crate::utils::geoip::geoip_lookup;
use actix_web::{http, web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error;
use std::sync::Arc;
use ulid::Ulid;
use woothee::parser::Parser;

fn generate_analytics_js(cid: &str, app_url: &str) -> String {
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
    use crate::schema::collectors::dsl::*;

    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let new_collector = Collector {
        id: Ulid::new().to_string(),
        origin: origin_str.to_string(),
        country: lookup_country.to_string(),
        city: lookup_city.to_string(),
        os: os_option,
        browser: browser_option,
        timestamp: Utc::now().naive_utc(),
    };

    diesel::insert_into(collectors)
        .values(&new_collector)
        .execute(&mut conn)?;

    Ok(new_collector.id)
}

pub async fn serve_collector_js(
    req: HttpRequest,
    config: web::Data<Arc<Config>>,
    pool: web::Data<DbPool>,
) -> impl Responder {
    let origin = req.headers().get("Origin").map_or_else(
        || "unknown".to_owned(),
        |v| v.to_str().unwrap_or("unknown").to_owned(),
    );

    let db_path = "data/GeoLite2-City.mmdb";
    let real_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok());
    let ip: &str = real_ip.unwrap_or("0.0.0.0");

    let mut os: Option<String> = None;
    let mut browser: Option<String> = None;

    if let Some(user_agent_string) = req.headers().get("User-Agent") {
        if let Ok(ua_string) = user_agent_string.to_str() {
            let parser = Parser::new();
            let result = parser.parse(ua_string);
            if let Some(ref parsed_result) = result {
                os = Some(parsed_result.os.to_string());
                browser = Some(parsed_result.name.to_string());
            }
        }
    }

    let (lookup_country, lookup_city) = match geoip_lookup(ip, db_path) {
        Ok((_country, _city)) => (_country.to_owned(), _city.to_owned()),
        Err(_) => ("Unknown".to_owned(), "Unknown".to_owned()),
    };

    let collector_result = web::block(move || {
        create_collector(
            &pool,
            &origin,
            &lookup_country,
            &lookup_city,
            os.clone(),
            browser.clone(),
        )
    })
    .await;

    match collector_result {
        Ok(collector_id) => match collector_id {
            Ok(id) => {
                let js_content = generate_analytics_js(&id, &config.app_url);
                HttpResponse::Ok()
                    .insert_header((http::header::CACHE_CONTROL, "public, max-age=1800")) // cache for 30 minutes
                    .content_type("application/javascript")
                    .body(js_content)
            }
            Err(e) => {
                eprintln!("Error creating collector: {}", e);
                HttpResponse::InternalServerError().finish()
            }
        },
        Err(e) => {
            eprintln!("Error serving collector JS: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

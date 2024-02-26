use crate::config::Config;
use crate::db::DbPool;
use crate::models::Collector;
use crate::utils::geoip::geoip_lookup;
use actix_web::{http, web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error;
use log::{info, warn};
use std::sync::Arc;
use ulid::Ulid;

fn generate_analytics_js(cid: &str, app_url: &str) -> String {
    format!(
        r#""use strict";
(function() {{
    var collectorId = "{}";
    var appUrl = "{}";



    function init() {{
        // check api to see if collector is expired
        // if expired, get a new collector id
        // if not expired, continue with the current collector id

        // function checkCollectorStatus() {{
        //     var url = new URL(appUrl + "/collectors/" + collectorId);
        //     fetch(url)
        //     .then(res => res.json())
        //     .then(data => {{
        //         console.log("📼", "collector refreshed");
        //     }})
        //     .catch(rejected => {{
        //         console.log("📼", "failed to check collector status, continuing with existing collector.");
        //     }});
        // }}

        // checkCollectorStatus();

        document.addEventListener('click', function(event) {{
            if (event.target.tagName === 'A') {{
                var target = event.target.getAttribute('target');
                var href = event.target.getAttribute('href');
                
                if (target === '_blank') {{
                    stats_collect('external_link_click', href);
                }} else {{
                    var url = new URL(href, window.location.origin);
                    stats_collect('link_click', url.href);
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

    async function send(type = "pageview", url_override = null) {{
        var url = new URL(appUrl + "/collect");

        url.searchParams.set('collector_id', collectorId);
        url.searchParams.set('name', type);
        url.searchParams.set('url', url_override || window.location.href);

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
) -> Result<String, Error> {
    use crate::schema::collectors::dsl::*;

    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let new_collector = Collector {
        id: Ulid::new().to_string(),
        origin: origin_str.to_string(),
        country: lookup_country.to_string(),
        city: lookup_city.to_string(),
        timestamp: Utc::now().naive_utc(),
    };

    // Execute the insertion query
    diesel::insert_into(collectors)
        .values(&new_collector)
        .execute(&mut conn)?;

    // Return the ID of the newly inserted collector
    Ok(new_collector.id)
}

pub async fn serve_collector_js(
    req: HttpRequest,
    config: web::Data<Arc<Config>>,
    pool: web::Data<DbPool>,
) -> impl Responder {
    let origin = req.headers().get("Origin").map_or_else(
        || "unknown".to_string(),
        |v| v.to_str().unwrap_or("unknown").to_string(),
    );

    // Perform GeoIP lookup, you can get this database for free at https://maxmind.com
    // Place it in the /data folder for this to work.
    let db_path = "data/GeoLite2-City.mmdb";

    let real_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok());
    let ip: &str = match real_ip {
        Some(s) => s,
        None => "0.0.0.0",
    };

    let (lookup_country, lookup_city) = match geoip_lookup(ip, db_path) {
        Ok((_country, _city)) => (_country, _city),
        Err(e) => {
            warn!("Error during GeoIP lookup: {}", e);
            ("Unknown".to_string(), "Unknown".to_string())
        }
    };

    let collector_result =
        web::block(move || create_collector(&pool, &origin, &lookup_country, &lookup_city)).await;

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

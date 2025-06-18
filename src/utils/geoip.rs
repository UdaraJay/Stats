use maxminddb::geoip2::City;
use maxminddb::Reader;
use log::{warn, error, debug};
use std::net::IpAddr;

pub fn geoip_lookup(
    ip: &str,
    db_path: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    debug!("🌍 Starting GeoIP lookup - IP: {}, DB: {}", ip, db_path);
    
    let reader = match Reader::open_readfile(db_path) {
        Ok(reader) => {
            debug!("✅ GeoIP database opened successfully: {}", db_path);
            reader
        }
        Err(e) => {
            error!("❌ Failed to open GeoIP database: {} - Error: {}", db_path, e);
            return Err(e.into());
        }
    };

    let ip_addr: IpAddr = match ip.parse() {
        Ok(addr) => {
            debug!("✅ IP address parsed successfully: {}", addr);
            addr
        }
        Err(e) => {
            error!("❌ Invalid IP address format: {} - Error: {}", ip, e);
            return Err(e.into());
        }
    };

    match reader.lookup::<City<'_>>(ip_addr) {
        Ok(lookup_city) => {
            let country_name = lookup_city
                .country
                .and_then(|c| c.names)
                .and_then(|mut names| names.remove("en"))
                .unwrap_or_else(|| {
                    debug!("⚠️ Country name not found for IP: {}", ip);
                    "Unknown"
                });

            let city_name = lookup_city
                .city
                .and_then(|c| c.names)
                .and_then(|mut names| names.remove("en"))
                .unwrap_or_else(|| {
                    debug!("⚠️ City name not found for IP: {}", ip);
                    "Unknown"
                });

            debug!("✅ GeoIP lookup successful - IP: {}, Country: {}, City: {}", 
                   ip, country_name, city_name);
            Ok((country_name.to_string(), city_name.to_string()))
        }
        Err(e) => {
            warn!("⚠️ GeoIP lookup failed for IP: {} - Error: {}", ip, e);
            Err("GeoIP lookup failed".into())
        }
    }
}

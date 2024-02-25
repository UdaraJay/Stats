use maxminddb::geoip2::City;
use maxminddb::Reader;
use std::net::IpAddr;

pub fn geoip_lookup(
    ip: &str,
    db_path: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let reader = Reader::open_readfile(db_path)?;
    let ip: IpAddr = ip.parse()?;

    if let Ok(lookup_city) = reader.lookup::<City<'_>>(ip) {
        let country_name = lookup_city
            .country
            .and_then(|c| c.names)
            .and_then(|mut names| names.remove("en"))
            .unwrap_or_else(|| "Unknown");

        let city_name = lookup_city
            .city
            .and_then(|c| c.names)
            .and_then(|mut names| names.remove("en"))
            .unwrap_or_else(|| "Unknown");

        Ok((country_name.to_string(), city_name.to_string()))
    } else {
        Err("GeoIP lookup failed".into())
    }
}

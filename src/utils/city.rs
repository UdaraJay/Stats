use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::Mutex;
use strsim::jaro_winkler;

struct CityInfo {
    latitude: f64,
    longitude: f64,
}

static CITY_MAP: Lazy<HashMap<String, CityInfo>> =
    Lazy::new(|| load_city_data("data/cities5000.txt").expect("Failed to load city data"));

static SEARCH_CACHE: Lazy<Mutex<HashMap<String, Option<(f64, f64)>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn load_city_data<P: AsRef<Path>>(path: P) -> io::Result<HashMap<String, CityInfo>> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut city_map: HashMap<String, CityInfo> = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() > 4 {
            let city_name = parts[2].to_string();
            let latitude = parts[4].parse::<f64>().unwrap_or(0.0);
            let longitude = parts[5].parse::<f64>().unwrap_or(0.0);
            city_map.insert(
                city_name,
                CityInfo {
                    latitude,
                    longitude,
                },
            );
        }
    }

    Ok(city_map)
}

pub fn get_city_coordinates(city_name: &str) -> Option<(f64, f64)> {
    if city_name == "Unknown" {
        return None;
    }

    // First, try to get the result from the cache
    {
        let cache = SEARCH_CACHE.lock().unwrap();
        if let Some(cached_result) = cache.get(city_name) {
            return *cached_result;
        }
    }

    // Find direct match
    if let Some(info) = CITY_MAP.get(city_name) {
        return Some((info.latitude, info.longitude));
    }

    // Find closest match
    let mut best_match: Option<&CityInfo> = None;
    let mut highest_similarity = 0.0;

    for (key, info) in CITY_MAP.iter() {
        let similarity = jaro_winkler(city_name, key);
        if similarity > highest_similarity {
            highest_similarity = similarity;
            best_match = Some(info);
        }
    }

    let result = if highest_similarity > 0.8 {
        best_match.map(|info| (info.latitude, info.longitude))
    } else {
        None
    };

    {
        let mut cache = SEARCH_CACHE.lock().unwrap();
        cache.insert(city_name.to_string(), result);
    }

    result
}

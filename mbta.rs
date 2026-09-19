use serde::Deserialize;
use apiresponse::ApiResponse;
use serde_json::json;

#[derive(Deserialize)]
#[derive(Debug)]
pub struct Train {
    pub lat: Option<f64>,
    pub long: Option<f64>,
    pub route_id: Option<String>,
    pub stop_id: Option<String>,
    pub label: String,
}

pub async fn fetch_vehicles(client: &reqwest::Client) -> Result<Vec<Train>, Box<dyn std::error::Error>> {
    let url = "https://api-v3.mbta.com/vehicles";
    let response = client.get(url).send().await?;
    let body = response.text().await?;
    let mut trains: Vec<Train> = Vec::new();
    
    let json: serde_json::Value = serde_json::from_str(&body)?;
    if let Some(vehicles) = json["data"].as_array() {
        for vehicle in vehicles {
            let train = Train {
                lat: vehicle["attributes"]["latitude"].as_f64(),
                long: vehicle["attributes"]["longitude"].as_f64(),
                route_id: vehicle["relationships"]["route"]["data"]["id"].as_str().map(|s| s.to_string()),
                stop_id: vehicle["relationships"]["stop"]["data"]["id"].as_str().map(|s| s.to_string()),
                label: vehicle["attributes"]["label"].as_str().unwrap().to_string(),
            };

            trains.push(train);
        }
    }
    
    Ok(trains)
}

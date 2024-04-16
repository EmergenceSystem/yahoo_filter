use actix_web::{post, App, HttpServer, HttpResponse, Responder};
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::from_str;
use std::string::String;
use url::form_urlencoded;
use embryo::{Embryo, EmbryoList};
use std::collections::HashMap;
use std::time::{Instant, Duration};

static SEARCH_URL: &str = "https://fr.search.yahoo.com/search?p=";
static EXCLUDED_CONTENT: [&str; 1] = ["yahoo"];

#[post("/query")]
async fn query_handler(body: String) -> impl Responder {
    let embryo_list = generate_embryo_list(body).await;
    let response = EmbryoList { embryo_list };
    HttpResponse::Ok().json(response)
}

async fn generate_embryo_list(json_string: String) -> Vec<Embryo> {
    let search: HashMap<String,String> = from_str(&json_string).expect("Can't parse JSON");
    let value = match search.get("value") {
        Some(v) => v,
        None => "",
    };
    let timeout : u64 = match search.get("timeout") {
        Some(t) => t.parse().expect("Can't parse as u64"),
        None => 10,
    };
    let encoded_search: String = form_urlencoded::byte_serialize(value.as_bytes()).collect();
    let search_url = format!("{}{}", SEARCH_URL, encoded_search);
    println!("{}", search_url);
    let response = Client::new().get(&search_url).send().await;

    match response {
        Ok(response) => {
            if let Ok(body) = response.text().await {
                let embryo_list = extract_links_from_results(body, timeout);
                return embryo_list;
            }
        }
        Err(e) => eprintln!("Error fetching search results: {:?}", e),
    }

    Vec::new()
}

fn extract_links_from_results(html: String, timeout_secs: u64) -> Vec<Embryo> {
    let mut embryo_list = Vec::new();
    let fragment = Html::parse_document(&html);
    let selector = Selector::parse(".relsrch").unwrap();

    let start_time = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    for element in fragment.select(&selector) {
        if start_time.elapsed() >= timeout {
            return embryo_list;
        }
        let anchor_selector = Selector::parse(".compTitle a").unwrap();

        let link = element.select(&anchor_selector)
            .next()
            .and_then(|elem| elem.value().attr("href"))
            .unwrap_or_default();


        if EXCLUDED_CONTENT.iter().any(|excluded| link.contains(excluded)) || !link.starts_with("http")
        {
            continue;
        }

        let span_selector = Selector::parse(".fc-falcon").unwrap();
        let resume = element.select(&span_selector)
            .next()
            .map(|elem| elem.text().collect::<Vec<_>>().join(""))
            .unwrap_or_default()
            .trim().to_string();

        let mut embryo_properties = HashMap::new();
        embryo_properties.insert("url".to_string(),link.to_string());
        embryo_properties.insert("resume".to_string(),resume.to_string());
        let embryo = Embryo {
            properties: embryo_properties,
        };

        embryo_list.push(embryo);
    }

    embryo_list
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    match em_filter::find_port().await {
        Some(port) => {
            let filter_url = format!("http://localhost:{}/query", port);
            println!("Filter registrer: {}", filter_url);
            em_filter::register_filter(&filter_url).await;
            HttpServer::new(|| App::new().service(query_handler))
                .bind(format!("127.0.0.1:{}", port))?.run().await?;
        },
        None => {
            println!("Can't start");
        },
    }
    Ok(())
}

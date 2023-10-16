use std::net::IpAddr;

use reqwest::Client;

use crate::error::Error;

pub async fn public_ip() -> Result<IpAddr, Error> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()?;

    let urls = [
        "https://icanhazip.tacticalrmm.io/",
        "https://icanhazip.com",
        "https://ifconfig.co/ip",
    ];

    let futures = urls
        .into_iter()
        .map(|url| Box::pin(handle_url(&client, url)));
    let ip = futures::future::select_ok(futures).await?.0;

    Ok(ip)
}

async fn handle_url(client: &Client, url: &str) -> Result<IpAddr, Error> {
    let request = client.get(url);
    if let Ok(response) = request.send().await {
        if let Ok(body) = response.text().await {
            let ip_stripped = body.trim().to_string();
            if let Ok(parsed_ip) = ip_stripped.parse::<IpAddr>() {
                if parsed_ip.is_ipv4() {
                    return Ok(parsed_ip);
                } else {
                    if let Ok(response_v4) = client.get("https://ifconfig.me/ip").send().await {
                        if let Ok(body_v4) = response_v4.text().await {
                            let ipv4_stripped = body_v4.trim().to_string();
                            if let Ok(ipv4_parsed_ip) = ipv4_stripped.parse::<IpAddr>() {
                                return Ok(ipv4_parsed_ip);
                            }
                        }
                    }
                }
            }
        }
    }
    Err(Error::NotFoundPublicIp)
}

#[tokio::test]
async fn test_public_ip() {
    println!("{:?}", public_ip().await);
}

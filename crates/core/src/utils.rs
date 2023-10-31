use std::net::IpAddr;

use crate::error::Error;

pub async fn public_ip() -> Result<IpAddr, Error> {
    let ip = public_ip::addr().await.ok_or(Error::NotFoundPublicIp)?;

    Ok(ip)
}

#[tokio::test]
async fn test_public_ip() {
    println!("{:?}", public_ip().await);
}

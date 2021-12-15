//! here be database stuff

use fred::prelude::*;

pub async fn setup() -> Result<RedisClient, crate::Error> {
    let config = RedisConfig::default();
    let policy = ReconnectPolicy::new_exponential(0, 100, 30_000, 2);
    let client = RedisClient::new(config);
    let _ = client.connect(Some(policy));
    let _ = client.wait_for_connect().await?;

    Ok(client)
}

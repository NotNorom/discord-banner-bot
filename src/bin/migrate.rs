use discord_banner_bot::{database::Database, Settings};

#[tokio::main]
async fn main() -> Result<(), discord_banner_bot::Error> {
    Settings::init()?;
    let settings = Settings::get();
    let database = Database::setup(&settings.database).await?;

    database.client();

    Ok(())
}

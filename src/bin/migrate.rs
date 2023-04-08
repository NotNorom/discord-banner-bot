use discord_banner_bot::{
    database::{guild_schedule::GuildSchedule, Database},
    utils::start_logging,
    Settings,
};
use fred::prelude::{KeysInterface, SetsInterface};

#[tokio::main]
async fn main() -> Result<(), discord_banner_bot::Error> {
    Settings::init()?;
    let settings = Settings::get();

    start_logging("discord_banner_bot=debug,reqwest=info,poise=info,serenity=info,fred=info,warn");
    let database = Database::setup(&settings.database).await?;
    let client = database.client();

    let known_guilds: Vec<u64> = client.smembers("dbb:known_guilds").await?;
    println!("Renaming {} guild_schedules", known_guilds.len());

    for (idx, guild_id) in known_guilds.iter().enumerate() {
        println!("Renaming #{idx} guild: {guild_id}");
        client
            .rename(
                format!("dbb:{guild_id}"),
                format!("dbb:active_schedule:{guild_id}"),
            )
            .await?;
    }

    client.rename("dbb:known_guilds", "dbb:active_schedules").await?;

    for (idx, guild_id) in known_guilds.iter().enumerate() {
        println!("Checking #{idx} guild: {guild_id}");
        let schedule = database.get::<GuildSchedule>(*guild_id).await;
        assert!(schedule.is_ok());
    }

    // database.set_bot_version("0.4.0").await?;
    // database.set_db_version("0.4.0").await?;

    Ok(())
}

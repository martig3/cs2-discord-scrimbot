use anyhow::Result;
use poise::command;
use regex::Regex;
use serenity::utils::MessageBuilder;

use crate::{
    utils::{self, write_to_file},
    Context,
};

#[command(slash_command, guild_only)]
pub(crate) async fn steam_id(
    context: Context<'_>,
    #[description = "Your SteamID i.e. STEAM_0:1:12345678"] steam_id: String,
) -> Result<()> {
    let steam_id_regex = Regex::new("^STEAM_[0-5]:[01]:\\d+$").unwrap();
    if !steam_id_regex.is_match(&steam_id) {
        context.say("Invalid SteamID formatting. Please follow this example: `/steamid STEAM_0:1:12345678`").await?;
        return Ok(());
    }
    let steam_ids = {
        let mut steam_ids = context.data().steam_id_cache.lock().await;
        steam_ids.insert(*context.author().id.as_u64(), String::from(&steam_id));
        steam_ids.clone()
    };
    write_to_file(
        String::from("data/steam-ids.json"),
        serde_json::to_string(&steam_ids).unwrap(),
    )
    .await;
    let steamid_64 = utils::convert_steamid_to_64(&steam_id);
    let response = MessageBuilder::new()
        .push("Updated steamid for ")
        .mention(context.author())
        .push(" to `")
        .push(&steam_id)
        .push("`\n")
        .push_line("Your steam community profile (please double check this is correct):")
        .push_line(format!(
            "https://steamcommunity.com/profiles/{}",
            steamid_64
        ))
        .build();
    context.say(response).await?;
    Ok(())
}

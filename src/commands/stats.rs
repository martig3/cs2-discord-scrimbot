use poise::command;
use reqwest::header;

use crate::{
    utils::{format_stats, get_api_client, Stats},
    Context,
};
use anyhow::Result;

#[command(slash_command, guild_only, ephemeral)]
pub(crate) async fn handle_stats(
    context: Context<'_>,
    #[description = "Query type"] type_option: Option<String>,
    #[description = "How many months to go back"] months: Option<i32>,
    #[description = "Map"] map: Option<String>,
) -> Result<()> {
    let config = &context.data().config;
    let Some(api_config)  = &config.scrimbot_api_config else {
        context
            .say("The scrimbot-api integration has not been configured")
            .await?;
        return Ok(());
    };
    let client = get_api_client(api_config);
    let steam_ids = context.data().steam_id_cache.lock().await.clone();
    let Some(steam_id) = steam_ids.get(context.author().id.as_u64()) else {
        context
            .say("Cannot find your steamId, please assign one using the `/steamid` command")
            .await?;
        return Ok(());
    };
    let mut steam_id = steam_id.clone();
    steam_id.replace_range(6..7, "1");
    let mut options = Vec::new();
    let mut print_map = false;
    if let Some(type_option) = type_option {
        if type_option == "top10" {
            options.push(("steamid", steam_id));
        } else {
            options.push(("option", type_option));
            print_map = true
        }
    };
    if let Some(month_option) = months {
        options.push(("months", month_option.to_string()));
    };
    if let Some(map_option) = map {
        options.push(("map", map_option.to_string()));
    };

    let resp = client
        .get(&format!(
            "{}/api/stats",
            api_config.scrimbot_api_url.as_ref().unwrap()
        ))
        .query(options.as_slice())
        .send()
        .await
        .unwrap();
    if resp.status() != 200 {
        context
            .say("Something went wrong retrieving stats, please try again later")
            .await?;
        return Ok(());
    }
    let content = resp.text().await.unwrap();
    let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
    if stats.is_empty() {
        context.say("No stats found for this query").await?;
        return Ok(());
    }
    let guild_id = context.guild_id().unwrap();
    let stats_str =
        format_stats(&stats, &context, &steam_ids, guild_id.as_u64(), print_map).await?;
    context.say(stats_str).await?;
    Ok(())
}

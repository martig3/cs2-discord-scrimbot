use crate::{utils::write_to_file, Context};
use anyhow::Result;
use poise::command;
use serenity::utils::MessageBuilder;
#[command(
    slash_command,
    guild_only,
    ephemeral,
    default_member_permissions = "MODERATE_MEMBERS",
    subcommands("clear", "map")
)]
pub(crate) async fn admin(_context: Context<'_>) -> Result<()> {
    Ok(())
}

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Clear the queue")
)]
pub(crate) async fn clear(context: Context<'_>) -> Result<()> {
    {
        let mut user_queue = context.data().user_queue.lock().await;
        user_queue.clear();
    }
    context.say("Queue cleared").await?;
    Ok(())
}
#[command(
    slash_command,
    guild_only,
    ephemeral,
    subcommands("add_map", "remove_map")
)]
pub(crate) async fn map(_context: Context<'_>) -> Result<()> {
    Ok(())
}
#[command(slash_command, guild_only, ephemeral, rename = "add")]
pub(crate) async fn add_map(
    context: Context<'_>,
    #[description = "Map name"] map_name: String,
) -> Result<()> {
    let maps = context.data().maps.lock().await.clone();
    if maps.len() >= 26 {
        context.say("Unable to add map, max amount reached").await?;
        return Ok(());
    }
    if maps.contains(&map_name) {
        context.say("Unable to add map, already exists").await?;
        return Ok(());
    }
    let maps = {
        let mut maps = context.data().maps.lock().await;
        maps.push(String::from(&map_name));
        maps.clone()
    };
    write_to_file(
        String::from("data/maps.json"),
        serde_json::to_string(&maps).unwrap(),
    )
    .await;
    let response = MessageBuilder::new()
        .push("Added map: `")
        .push(&map_name)
        .push("`")
        .build();
    context.say(response).await?;
    Ok(())
}
#[command(slash_command, guild_only, ephemeral, rename = "remove")]
pub(crate) async fn remove_map(
    context: Context<'_>,
    #[description = "Map name"] map_name: String,
) -> Result<()> {
    let maps = context.data().maps.lock().await.clone();
    if !maps.contains(&map_name) {
        context
            .say(format!("Map `{}` is not in the map pool", map_name))
            .await?;
        return Ok(());
    }
    let maps = {
        let mut maps = context.data().maps.lock().await;
        let index = maps.iter().position(|m| m == &map_name).unwrap();
        maps.remove(index);
        maps.clone()
    };
    write_to_file(
        String::from("data/maps.json"),
        serde_json::to_string(&maps).unwrap(),
    )
    .await;
    context.say(format!("Removed map: `{}`", map_name)).await?;
    Ok(())
}

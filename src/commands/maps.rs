use anyhow::Result;
use poise::command;
use serenity::utils::MessageBuilder;

use crate::Context;

#[command(slash_command, guild_only, ephemeral)]
pub(crate) async fn maps(context: Context<'_>) -> Result<()> {
    let maps = context.data().maps.lock().await.clone();
    let map_str: String = maps
        .into_iter()
        .map(|map| format!("- `{}`\n", map))
        .collect();
    let response = MessageBuilder::new()
        .push_line("Current map pool:")
        .push(map_str)
        .build();
    context.say(response).await?;

    Ok(())
}

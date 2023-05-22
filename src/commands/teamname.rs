use anyhow::Result;
use poise::command;

use crate::{utils::write_to_file, Context};

#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Set your custom team name")
)]
pub(crate) async fn teamname(
    context: Context<'_>,
    #[description = "Team name"] team_name: String,
) -> Result<()> {
    if team_name.len() > 20 {
        context
            .say(&format!(
                "Team name is over the character limit ({}/20).",
                team_name.len() - 19
            ))
            .await?;
        return Ok(());
    }
    let team_names = {
        let mut team_names = context.data().team_names.lock().await;
        team_names.insert(*context.author().id.as_u64(), String::from(&team_name));
        team_names.clone()
    };
    write_to_file(
        String::from("data/teamnames.json"),
        serde_json::to_string(&team_names).unwrap(),
    )
    .await;
    context
        .say(&format!("Team name successfully set to `{}`", &team_name))
        .await?;
    Ok(())
}

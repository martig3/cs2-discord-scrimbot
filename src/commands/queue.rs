use crate::{utils::write_to_file, Context, State};
use anyhow::Result;
use poise::{command, serenity_prelude::Guild};
use serenity::utils::MessageBuilder;

#[command(
    slash_command,
    guild_only,
    ephemeral,
    subcommands("join", "leave", "list")
)]
pub(crate) async fn queue(_context: Context<'_>) -> Result<()> {
    Ok(())
}
#[command(
    slash_command,
    guild_only,
    description_localized("en-US", "Join the scrim queue")
)]
pub(crate) async fn join(
    context: Context<'_>,
    #[description = "Message"] message: Option<String>,
) -> Result<()> {
    let steam_id_cache = &context.data().steam_id_cache.lock().await.clone();
    if !steam_id_cache.contains_key(context.author().id.as_u64()) {
        let response = MessageBuilder::new()
            .push("SteamID not found for your discord user, \
                    please use `/steamid` command to assign one. Example: `/steamid STEAM_0:1:12345678` ")
            .push("https://steamid.io/ is an easy way to find your steamID for your account")
            .build();
        context
            .send(|m| m.ephemeral(true).content(response))
            .await?;
        return Ok(());
    }

    let validation: Option<&str> = {
        let user_queue = context.data().user_queue.lock().await;
        match user_queue {
            _ if user_queue.len() >= 10 => Some("Sorry, the queue is full"),
            _ if user_queue.contains(context.author()) => Some("You are already in the queue"),
            _ => None,
        }
    };
    if let Some(msg) = validation {
        context
            .send(|m| m.ephemeral(true).content(msg.to_string()))
            .await?;
        return Ok(());
    }

    let user_queue = {
        let mut user_queue = context.data().user_queue.lock().await;
        user_queue.push(context.author().clone());
        user_queue.clone()
    };
    write_to_file(
        String::from("data/queue.json"),
        serde_json::to_string(&user_queue).unwrap(),
    )
    .await;
    let response = MessageBuilder::new()
        .mention(context.author())
        .push(" has been added to the queue. Queue size: ")
        .push(user_queue.len().to_string())
        .push("/10")
        .build();
    context.say(response).await?;
    let queue_messages = {
        let queue_messages = &mut context.data().queue_messages.lock().await;
        if let Some(m) = message {
            let mut end = m.len();
            end = end.min(50);
            queue_messages.insert(
                *context.author().id.as_u64(),
                String::from(m[0..end].trim()),
            );
        }
        queue_messages.clone()
    };
    write_to_file(
        String::from("data/queue-messages.json"),
        serde_json::to_string(&queue_messages).unwrap(),
    )
    .await;

    let config = &context.data().config;
    if let Some(role_id) = config.discord.assign_role_id {
        if let Ok(value) = context
            .author()
            .has_role(&context, context.guild_id().unwrap(), role_id)
            .await
        {
            if !value {
                let guild = Guild::get(&context, context.guild_id().unwrap())
                    .await
                    .unwrap();
                if let Ok(mut member) = guild.member(&context, &context.author().id).await {
                    if let Err(err) = member.add_role(&context, role_id).await {
                        eprintln!("assign_role_id exists but cannot add role to user, check bot permissions");
                        eprintln!("{:?}", err);
                    }
                }
            }
        }
    }
    Ok(())
}
#[command(
    slash_command,
    guild_only,
    description_localized("en-US", "Leave the scrim queue")
)]
pub(crate) async fn leave(context: Context<'_>) -> Result<()> {
    let state = &context.data().state.lock().await.clone();
    if state != &State::Queue {
        context
            .say("Cannot `/leave` the queue after `/start`")
            .await?;
        return Ok(());
    }
    let validation = {
        let user_queue = context.data().user_queue.lock().await;
        match user_queue {
            _ if !user_queue.contains(&context.author()) => {
                Some("You are not in the queue. Use `/join` to join the queue.")
            }
            _ => None,
        }
    };
    if let Some(msg) = validation {
        context.send(|m| m.ephemeral(true).content(msg)).await?;
        return Ok(());
    }
    let user_queue = {
        let mut user_queue = context.data().user_queue.lock().await;
        let index = user_queue
            .iter()
            .position(|r| r.id == context.author().id)
            .unwrap();
        user_queue.remove(index);
        user_queue.clone()
    };
    write_to_file(
        String::from("data/queue.json"),
        serde_json::to_string(&user_queue).unwrap(),
    )
    .await;
    let response = MessageBuilder::new()
        .mention(context.author())
        .push(" has left the queue. Queue size: ")
        .push(user_queue.len().to_string())
        .push("/10")
        .build();
    context.say(response).await?;
    let queued_msgs = {
        let mut queued_msgs = context.data().queue_messages.lock().await;
        if queued_msgs.get(context.author().id.as_u64()).is_some() {
            queued_msgs.remove(context.author().id.as_u64());
        }
        queued_msgs.clone()
    };
    write_to_file(
        String::from("data/queue-messages.json"),
        serde_json::to_string(&queued_msgs.clone()).unwrap(),
    )
    .await;
    Ok(())
}
#[command(
    slash_command,
    guild_only,
    ephemeral,
    description_localized("en-US", "Display the queue")
)]
pub(crate) async fn list(context: Context<'_>) -> Result<()> {
    let user_queue = &context.data().user_queue.lock().await.clone();
    let queue_msgs = &context.data().queue_messages.lock().await.clone();
    let mut user_name = String::new();
    for u in user_queue {
        user_name.push_str(format!("\n- @{}", u.name).as_str());
        if let Some(value) = queue_msgs.get(u.id.as_u64()) {
            user_name.push_str(format!(": `{}`", value).as_str());
        }
    }
    let response = MessageBuilder::new()
        .push("Current queue size: ")
        .push(&user_queue.len())
        .push("/10")
        .push(user_name)
        .build();
    context.say(response).await?;
    Ok(())
}

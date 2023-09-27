use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use poise::{
    command,
    serenity_prelude::{ButtonStyle, InteractionResponseType, ReactionType, User},
};
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serenity::{
    builder::{CreateActionRow, CreateButton, CreateSelectMenu, CreateSelectMenuOption},
    futures::StreamExt,
    model::application::interaction::message_component::MessageComponentInteraction,
    utils::MessageBuilder,
};

use crate::{
    utils::{get_api_client, list_teams, reset_draft, user_in_queue, Stats},
    Context, State,
};

#[derive(Serialize, Deserialize)]
pub struct ServerInfoResponse {
    pub game: Option<String>,
    pub id: String,
    pub ip: String,
    pub ports: Ports,
    pub location: Option<String>,
    pub custom_domain: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Ports {
    pub game: i64,
    pub gotv: i64,
}

#[command(
    slash_command,
    guild_only,
    description_localized("en-US", "Start scrim setup")
)]
pub(crate) async fn start(context: Context<'_>) -> Result<()> {
    let in_queue = user_in_queue(&context, None).await?;
    if !in_queue {
        return Ok(());
    }

    let queue = context.data().user_queue.lock().await.clone();
    if queue.len() < 10 {
        context
            .send(|m| m.ephemeral(true).content("The queue is not full yet"))
            .await?;
        return Ok(());
    }

    {
        let mut state = context.data().state.lock().await;
        if *state != State::Queue {
            context
                .send(|m| m.ephemeral(true).content("Setup has already started"))
                .await?;
            return Ok(());
        }
        *state = State::Ready;
    }
    let content = list_ready(&context).await?;

    let msg = context
        .send(|m| {
            m.content(content)
                .components(|c| c.add_action_row(create_ready_check_action_row()))
        })
        .await?;

    let mut cib = msg
        .message()
        .await?
        .await_component_interactions(&context)
        .timeout(Duration::from_secs(60 * 3))
        .build();
    loop {
        let opt = cib.next().await;
        match opt {
            Some(mci) => {
                let completed = handle_ready(&context, &mci).await?;
                if completed {
                    break;
                }
            }
            None => {
                msg.edit(context, |m| {
                    m.content("Start process timed out. Start again when all users are present using `/start`")
                }).await?;
                reset_draft(&context).await?;
                return Ok(());
            }
        }
    }
    {
        let mut state = context.data().state.lock().await;
        *state = State::MapPick;
    }
    let map_list = context.data().maps.lock().await.clone();
    msg.edit(context, |f| {
        f.content("Map vote phase: vote for 1 or more maps")
            .components(|c| c.add_action_row(create_map_action_row(map_list.clone())))
    })
    .await?;

    let mut cib = msg
        .message()
        .await?
        .await_component_interactions(&context)
        .timeout(Duration::from_secs(60))
        .build();
    loop {
        let opt = cib.next().await;
        match opt {
            Some(mci) => {
                let completed = handle_map_pick(&context, &mci).await?;
                if completed {
                    break;
                }
            }
            None => {
                break;
            }
        }
    }
    let selected_map = calc_selected_map(&context).await?;
    {
        let mut draft = context.data().draft.lock().await;
        draft.selected_map = selected_map.clone();
    };
    {
        let mut state = context.data().state.lock().await;
        *state = State::DraftTypePick;
    }
    msg.edit(context, |m| {
        m.components(|c| c.add_action_row(create_draft_type_action_row()))
            .content(format!(
                "Map vote has concluded. `{}` will be played.\n\nSelect draft option:",
                selected_map
            ))
    })
    .await?;
    let mut cib = msg
        .message()
        .await?
        .await_component_interactions(&context)
        .timeout(Duration::from_secs(60 * 10))
        .build();
    while let Some(mci) = cib.next().await {
        let in_queue = user_in_queue(&context, Some(&mci)).await?;
        if !in_queue {
            return Ok(());
        }
        let state = context.data().state.lock().await.clone();
        match state {
            State::CaptainPick => handle_captain_pick(&context, &mci).await?,
            State::DraftTypePick => handle_draft_type(&context, &mci).await?,
            State::Draft => handle_draft(&context, &mci).await?,
            State::SidePick => {
                let completed = handle_sidepick(&context, &mci).await?;
                if completed {
                    break;
                }
            }
            _ => return Err(anyhow!("Something went wrong")),
        };
    }

    start_server(&context).await?;

    Ok(())
}

async fn handle_ready(context: &Context<'_>, mci: &MessageComponentInteraction) -> Result<bool> {
    {
        let mut queue = context.data().ready_queue.lock().await;
        match mci.data.custom_id.as_str() {
            "ready" => {
                queue.push(mci.user.clone());
            }
            "unready" => {
                if let Some(pos) = queue.iter().position(|u| u.id == mci.user.id) {
                    queue.remove(pos);
                }
            }
            _ => return Err(anyhow!("Unable to parse ready button response")),
        };
    };
    let content = list_ready(context).await?;
    mci.create_interaction_response(&context, |r| {
        r.kind(InteractionResponseType::UpdateMessage)
            .interaction_response_data(|d| d.content(content))
    })
    .await?;
    let ready_queue = context.data().ready_queue.lock().await.clone();
    let user_queue = context.data().user_queue.lock().await.clone();
    if ready_queue.len() != user_queue.len() {
        return Ok(false);
    }
    Ok(true)
}

async fn list_ready(context: &Context<'_>) -> Result<String> {
    let ready_queue = context.data().ready_queue.lock().await.clone();
    let queue = context.data().user_queue.lock().await.clone();
    let ready_list: String = queue
        .into_iter()
        .map(|u| {
            let is_ready = if ready_queue.contains(&u) { '✔' } else { ' ' };
            format!("{} {} \n", is_ready, u)
        })
        .collect();
    Ok(MessageBuilder::new()
        .push_line("Ready check:")
        .push_line(ready_list)
        .build())
}

async fn handle_sidepick(context: &Context<'_>, mci: &MessageComponentInteraction) -> Result<bool> {
    let draft = context.data().draft.lock().await.clone();
    if mci.user != draft.captain_b.unwrap() {
        mci.create_interaction_response(context, |m| {
            m.interaction_response_data(|d| {
                d.ephemeral(true)
                    .content("You are not the captain of the team picking sides")
            })
        })
        .await?;
        return Ok(false);
    }
    let option = &mci.data.custom_id;
    {
        let mut draft = context.data().draft.lock().await;
        draft.team_b_start_side = option.clone();
    };
    mci.create_interaction_response(&context, |r| {
        r.kind(InteractionResponseType::UpdateMessage)
            .interaction_response_data(|d| d.components(|c| c))
    })
    .await?;
    Ok(true)
}

async fn handle_draft(context: &Context<'_>, mci: &MessageComponentInteraction) -> Result<()> {
    let draft = context.data().draft.lock().await.clone();
    if mci.user.id != draft.current_picker.unwrap().id {
        mci.create_interaction_response(context, |m| {
            m.interaction_response_data(|d| {
                d.ephemeral(true)
                    .content("You are not the current draft picker")
            })
        })
        .await?;
        return Ok(());
    }
    let user_id = mci.data.values.get(0).unwrap();
    println!("user_id: {}", user_id);
    let user_id = user_id.parse::<u64>()?;
    let queue = context.data().user_queue.lock().await.clone();
    let user = queue.iter().find(|u| u.id.0 == user_id).unwrap().clone();
    let action_msg = MessageBuilder::new()
        .mention(&mci.user)
        .push(" picked ")
        .mention(&user)
        .build();
    let draft = {
        let mut draft = context.data().draft.lock().await;
        if draft.captain_a.as_ref().unwrap().id == mci.user.id {
            draft.team_a.push(user);
            draft.current_picker = draft.captain_b.clone();
        } else {
            draft.team_b.push(user);
            draft.current_picker = draft.captain_a.clone();
        }
        draft.clone()
    };
    let team_names = context.data().team_names.lock().await.clone();
    if draft.team_a.len() + draft.team_b.len() != queue.len() {
        let resp = MessageBuilder::new()
            .push_line(action_msg)
            .push_line("")
            .push_line(list_teams(&draft, &team_names))
            .push("It is ")
            .mention(&draft.current_picker.unwrap())
            .push(" turn to pick:")
            .build();
        let remaining_users = get_remaining_users(context).await?;
        mci.create_interaction_response(&context, |r| {
            r.kind(InteractionResponseType::UpdateMessage)
                .interaction_response_data(|d| {
                    d.content(resp)
                        .components(|c| c.add_action_row(create_user_action_row(remaining_users)))
                })
        })
        .await?;
        return Ok(());
    }
    let team_names = context.data().team_names.lock().await.clone();
    init_sidepick_state(context, mci, Some(list_teams(&draft, &team_names))).await?;
    Ok(())
}

async fn init_sidepick_state(
    context: &Context<'_>,
    mci: &MessageComponentInteraction,
    msg_prefix: Option<String>,
) -> Result<()> {
    {
        let mut state = context.data().state.lock().await;
        *state = State::SidePick;
    }
    let draft = context.data().draft.lock().await.clone();
    let resp = MessageBuilder::new()
        .push_line(msg_prefix.unwrap_or(String::new()))
        .push_line("")
        .mention(&draft.captain_b.unwrap())
        .push(" select starting side on `")
        .push(draft.selected_map)
        .push("`")
        .build();
    mci.create_interaction_response(&context, |r| {
        r.kind(InteractionResponseType::UpdateMessage)
            .interaction_response_data(|d| {
                d.content(resp)
                    .components(|c| c.add_action_row(create_sidepick_action_row()))
            })
    })
    .await?;
    Ok(())
}

async fn get_remaining_users(context: &Context<'_>) -> Result<Vec<User>> {
    let draft = context.data().draft.lock().await.clone();
    let remaining_users: Vec<User> = context
        .data()
        .user_queue
        .lock()
        .await
        .clone()
        .into_iter()
        .filter(|user| !draft.team_a.contains(user) && !draft.team_b.contains(user))
        .collect();
    Ok(remaining_users)
}

async fn calc_selected_map(context: &Context<'_>) -> Result<String> {
    let votes = context.data().draft.lock().await.clone().map_votes;
    let vote_map = votes
        .into_values()
        .fold(HashMap::<String, i32>::new(), |mut accum, item| {
            for m in item {
                if let Some(c) = accum.get(&m) {
                    let nc = c + 1;
                    accum.insert(m, nc);
                } else {
                    accum.insert(m, 1);
                }
            }
            accum
        });
    let Some(max) = vote_map.values().max() else {
        return Err(anyhow!("No map votes were submitted"));
    };
    let max_maps: Vec<String> = vote_map
        .iter()
        .filter(|item| item.1 >= max)
        .map(|item| item.0.clone())
        .collect();
    if max_maps.len() == 1 {
        return Ok(max_maps.first().unwrap().to_string());
    }

    let map = &max_maps
        .get(rand::thread_rng().gen_range(0, max_maps.len()))
        .unwrap();

    Ok(map.to_string())
}

async fn handle_draft_type(
    context: &Context<'_>,
    mci: &Arc<MessageComponentInteraction>,
) -> Result<()> {
    let option = &mci.data.custom_id;
    {
        let mut state = context.data().state.lock().await;
        match option.as_str() {
            "autodraft" => {
                handle_autodraft(context, mci).await?;
            }
            "manualdraft" => {
                *state = State::CaptainPick;
                mci.create_interaction_response(&context, |r| {
                    r.kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|d| {
                            d.content(
                                "Manual draft selected, 2 players must volunteer to be captains:",
                            )
                            .components(|c| c.add_action_row(create_captain_action_row()))
                        })
                })
                .await?;
            }
            _ => return Err(anyhow!("invalid draft type")),
        }
    }
    Ok(())
}

fn create_captain_action_row() -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut autdraft_button = CreateButton::default();
    autdraft_button.custom_id("captain");
    autdraft_button.label("Become Captain");
    autdraft_button.style(ButtonStyle::Success);
    autdraft_button.emoji('🎖');
    ar.add_button(autdraft_button);
    ar
}

pub async fn handle_map_pick(
    context: &Context<'_>,
    mci: &Arc<MessageComponentInteraction>,
) -> Result<bool> {
    let in_queue = user_in_queue(context, Some(mci)).await?;
    if !in_queue {
        return Ok(false);
    }
    let maps_selected = &mci.data.values;
    let map_votes = {
        let mut draft = context.data().draft.lock().await;
        draft
            .map_votes
            .insert(mci.user.clone(), maps_selected.clone());
        draft.map_votes.clone()
    };
    let queue_len = context.data().user_queue.lock().await.clone().len();

    mci.create_interaction_response(context, |r| {
        r.kind(InteractionResponseType::DeferredUpdateMessage)
            .interaction_response_data(|d| d)
    })
    .await?;

    if queue_len == map_votes.len() {
        return Ok(true);
    }

    Ok(false)
}

pub fn create_map_action_row(map_list: Vec<String>) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut menu = CreateSelectMenu::default();
    menu.custom_id("map_select");
    menu.placeholder("Pick maps");
    let map_len = map_list.len();
    let mut options = Vec::new();
    for map_name in map_list {
        options.push(create_menu_option(
            &map_name,
            &map_name.to_ascii_lowercase(),
        ))
    }
    menu.options(|f| f.set_options(options));
    menu.min_values(1);
    menu.max_values(map_len.try_into().unwrap());
    ar.add_select_menu(menu);
    ar
}
pub fn create_user_action_row(user_list: Vec<User>) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut menu = CreateSelectMenu::default();
    menu.custom_id("user_select");
    menu.placeholder("Pick user");
    let options: Vec<CreateSelectMenuOption> = user_list
        .iter()
        .map(|u| create_menu_option(&u.name, &u.id.0.to_string()))
        .collect();
    menu.options(|f| f.set_options(options));
    ar.add_select_menu(menu);
    ar
}

pub fn create_menu_option(label: &str, value: &str) -> CreateSelectMenuOption {
    let mut opt = CreateSelectMenuOption::default();
    // This is what will be shown to the user
    opt.label(label);
    // This is used to identify the selected value
    opt.value(value.to_ascii_lowercase());
    opt
}

pub fn create_draft_type_action_row() -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut autdraft_button = CreateButton::default();
    autdraft_button.custom_id("autodraft");
    autdraft_button.label("Auto Draft");
    autdraft_button.style(ButtonStyle::Primary);
    autdraft_button.emoji('🤖');
    let mut manual_button = CreateButton::default();
    manual_button.custom_id("manualdraft");
    manual_button.label("Manual Draft");
    manual_button.style(ButtonStyle::Secondary);
    manual_button.emoji('⚙');
    ar.add_button(autdraft_button);
    ar.add_button(manual_button);
    ar
}

pub fn create_sidepick_action_row() -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut ct_button = CreateButton::default();
    ct_button.custom_id("ct");
    ct_button.label("Counter-Terrorist");
    ct_button.emoji('🚨');
    ct_button.style(ButtonStyle::Primary);
    let mut t_button = CreateButton::default();
    t_button.custom_id("t");
    t_button.label("Terrorist");
    t_button.style(ButtonStyle::Secondary);
    t_button.emoji('💣');
    ar.add_button(ct_button);
    ar.add_button(t_button);
    ar
}

pub fn create_ready_check_action_row() -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut ready_button = CreateButton::default();
    ready_button.custom_id("ready");
    ready_button.label("Ready");
    ready_button.style(ButtonStyle::Success);
    let mut unready_button = CreateButton::default();
    unready_button.custom_id("unready");
    unready_button.label("Unready");
    unready_button.style(ButtonStyle::Danger);
    ar.add_button(ready_button);
    ar.add_button(unready_button);
    ar
}

async fn handle_captain_pick(
    context: &Context<'_>,
    mci: &MessageComponentInteraction,
) -> Result<()> {
    let in_queue = user_in_queue(&context, Some(&mci)).await?;
    if !in_queue {
        return Ok(());
    }
    let draft = context.data().draft.lock().await.clone();
    if let Some(user) = &draft.captain_a {
        if user.id == mci.user.id {
            mci.create_interaction_response(context, |m| {
                m.interaction_response_data(|d| {
                    d.ephemeral(true).content("You are already a captain")
                })
            })
            .await?;
            return Ok(());
        }
    }
    if let Some(user) = &draft.captain_b {
        if user.id == mci.user.id {
            mci.create_interaction_response(context, |m| {
                m.interaction_response_data(|d| {
                    d.ephemeral(true).content("You are already a captain")
                })
            })
            .await?;
            return Ok(());
        }
    }
    let draft = {
        let mut draft = context.data().draft.lock().await;
        match draft.captain_a {
            Some(_) => {
                draft.captain_b = Some(mci.user.clone());
                draft.team_b.push(mci.user.clone());
            }
            None => {
                draft.captain_a = Some(mci.user.clone());
                draft.team_a.push(mci.user.clone());
                draft.current_picker = Some(mci.user.clone())
            }
        }
        draft.clone()
    };
    if draft.captain_a.is_none() || draft.captain_b.is_none() {
        mci.create_interaction_response(&context, |r| {
            r.kind(InteractionResponseType::UpdateMessage)
                .interaction_response_data(|d| {
                    d.content(
                        MessageBuilder::new()
                            .mention(&mci.user)
                            .push(" is a captain, 1 more player must volunteer to be captain:")
                            .build(),
                    )
                })
        })
        .await?;
        return Ok(());
    }

    let queue = context.data().user_queue.lock().await.clone().len();
    if draft.team_a.len() + draft.team_b.len() == queue {
        init_sidepick_state(context, mci, None).await?;
        return Ok(());
    }
    {
        let mut state = context.data().state.lock().await;
        *state = State::Draft;
    }
    let team_names = context.data().team_names.lock().await.clone();
    let resp = MessageBuilder::new()
        .push_line("Draft phase starting.")
        .push_line(list_teams(&draft, &team_names))
        .push("It is ")
        .mention(&draft.current_picker.unwrap())
        .push(" turn to pick")
        .build();
    let remaining_users = get_remaining_users(context).await?;
    mci.create_interaction_response(&context, |r| {
        r.kind(InteractionResponseType::UpdateMessage)
            .interaction_response_data(|d| {
                d.content(resp)
                    .components(|c| c.add_action_row(create_user_action_row(remaining_users)))
            })
    })
    .await?;

    Ok(())
}

async fn handle_autodraft(
    context: &Context<'_>,
    mci: &Arc<MessageComponentInteraction>,
) -> Result<()> {
    let user_queue = context.data().user_queue.lock().await.clone();
    let steam_ids = context.data().steam_id_cache.lock().await.clone();
    let mut user_queue_steamids: HashMap<u64, String> = HashMap::new();
    let mut user_queue_user_ids: HashMap<String, u64> = HashMap::new();
    for user in user_queue.iter() {
        let mut steamid = steam_ids.get(user.id.as_u64()).unwrap().to_string();
        steamid = steamid.replacen("STEAM_0", "STEAM_1", 1);
        user_queue_steamids.insert(*user.id.as_u64(), steamid.clone());
        user_queue_user_ids.insert(steamid.clone(), *user.id.as_u64());
    }
    let steamids: String = user_queue_steamids
        .into_values()
        .map(|s| format!("{},", s))
        .collect();

    let config = context.data().config.clone();
    if config
        .scrimbot_api_config
        .as_ref()
        .unwrap()
        .scrimbot_api_user
        == None
        || config
            .scrimbot_api_config
            .as_ref()
            .unwrap()
            .scrimbot_api_password
            == None
    {
        context.send(|m| m.ephemeral(true).content("Sorry, the scrimbot-api user/password has not been configured. This option is unavailable.")).await?;
        return Ok(());
    }

    let client = get_api_client(&config.clone().scrimbot_api_config.unwrap());

    let resp = client
        .get(&format!(
            "{}/stats",
            &config
                .scrimbot_api_config
                .unwrap()
                .scrimbot_api_url
                .clone()
                .unwrap()
        ))
        .query(&[("steamids", &steamids), ("option", &"players".to_string())])
        .send()
        .await
        .unwrap();
    if resp.status() != 200 {
        let error = format!(
            "HTTP error on /api/stats with following params: steamids: {}, option: players",
            &steamids
        );
        return Err(anyhow!(error));
    }
    let content = resp.text().await.unwrap();
    let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
    if stats.is_empty() {
        mci.create_interaction_response(&context, |r| {
            r.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|d| {
                    d.ephemeral(true)
                        .content("No statistics found for any players, please use another option")
                })
        })
        .await?;
        return Ok(());
    }
    if stats.len() < 2 {
        mci.create_interaction_response(&context, |r| {
            r.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(
                    |d: &mut serenity::builder::CreateInteractionResponseData| {
                        d.ephemeral(true).content(
                        "Unable to find stats for at least 2 players. Please use another option",
                    )
                    },
                )
        })
        .await?;
        return Ok(());
    }
    let captain_a_user = user_queue
        .iter()
        .find(|user| {
            user.id.as_u64()
                == user_queue_user_ids
                    .get(&stats.get(0).unwrap().steamId)
                    .unwrap()
        })
        .unwrap();
    let captain_b_user = user_queue
        .iter()
        .find(|user| {
            user.id.as_u64()
                == user_queue_user_ids
                    .get(&stats.get(1).unwrap().steamId)
                    .unwrap()
        })
        .unwrap();
    let draft = {
        let mut draft = context.data().draft.lock().await.clone();
        draft.captain_a = Some(captain_a_user.clone());
        draft.team_a.push(captain_a_user.clone());
        draft.captain_b = Some(captain_b_user.clone());
        draft.team_b.push(captain_b_user.clone());
        draft.current_picker = Some(draft.captain_b.as_ref().unwrap().clone());
        for i in 2..stats.len() {
            if i % 2 == 0 {
                draft.team_b.push(
                    user_queue
                        .iter()
                        .find(|user| {
                            user.id.as_u64()
                                == user_queue_user_ids
                                    .get(&stats.get(i).unwrap().steamId)
                                    .unwrap()
                        })
                        .unwrap()
                        .clone(),
                );
                draft.current_picker = Some(draft.captain_a.as_ref().unwrap().clone())
            } else {
                draft.team_a.push(
                    user_queue
                        .iter()
                        .find(|user| {
                            user.id.as_u64()
                                == user_queue_user_ids
                                    .get(&stats.get(i).unwrap().steamId)
                                    .unwrap()
                        })
                        .unwrap()
                        .clone(),
                );
                draft.current_picker = Some(draft.captain_b.as_ref().unwrap().clone())
            }
        }
        draft.clone()
    };
    let team_names = context.data().team_names.lock().await.clone();
    if draft.team_a.len() + draft.team_b.len() != user_queue.len() {
        {
            let mut state = context.data().state.lock().await;
            *state = State::Draft;
        }
        let teams_str = list_teams(&draft, &team_names);
        let remaining_users = get_remaining_users(context).await?;
        let resp= MessageBuilder::new()
            .push("Unable to find stats for all players. Continue draft and pick the remaining players manually.\n\n")
            .push(teams_str)
            .push("\nIt is ")
            .mention(&draft.current_picker.clone().unwrap())
            .push(" turn to pick")
            .build();
        mci.create_interaction_response(&context, |r| {
            r.kind(InteractionResponseType::UpdateMessage)
                .interaction_response_data(|d| {
                    d.content(resp)
                        .components(|c| c.add_action_row(create_user_action_row(remaining_users)))
                })
        })
        .await?;
        return Ok(());
    }

    let teams_str = list_teams(&draft, &team_names);
    init_sidepick_state(context, mci, Some(teams_str)).await?;

    Ok(())
}

async fn start_server(context: &Context<'_>) -> Result<()> {
    println!("Launching server...");
    let response = MessageBuilder::new().push("Starting server...").build();
    let msg = context.send(|m| m.content(response)).await?;
    let draft = context.data().draft.lock().await.clone();
    let steam_ids = context.data().steam_id_cache.lock().await.clone();
    let mut team_a_steam_ids: Vec<String> = draft
        .team_a
        .iter()
        .map(|u| steam_ids.get(u.id.as_u64()).unwrap().to_string())
        .collect();
    for team_a_steam_id in &mut team_a_steam_ids {
        team_a_steam_id.replace_range(6..7, "1");
    }
    let mut team_a_steam_id_str: String =
        team_a_steam_ids.iter().map(|s| format!("{},", s)).collect();
    team_a_steam_id_str = String::from(&team_a_steam_id_str[..team_a_steam_id_str.len() - 1]);
    let mut team_b_steam_ids: Vec<String> = draft
        .team_b
        .iter()
        .map(|u| steam_ids.get(u.id.as_u64()).unwrap().to_string())
        .collect();
    for team_b_steam_id in &mut team_b_steam_ids {
        team_b_steam_id.replace_range(6..7, "1");
    }
    let mut team_b_steam_id_str: String =
        team_b_steam_ids.iter().map(|s| format!("{},", s)).collect();
    team_b_steam_id_str = String::from(&team_b_steam_id_str[..team_b_steam_id_str.len() - 1]);
    let team_ct: String;
    let team_t: String;
    if draft.team_b_start_side == "ct" {
        team_ct = team_b_steam_id_str;
        team_t = team_a_steam_id_str;
    } else {
        team_ct = team_a_steam_id_str;
        team_t = team_b_steam_id_str;
    }

    println!("Starting server with the following params:");
    println!("team1_steam_ids:'{}'", &team_t);
    println!("team2_steam_ids:'{}'", &team_ct);

    let config = &context.data().config;
    let client = reqwest::Client::new();
    let dathost_username = &config.dathost.username;
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    let server_id = &config.dathost.server_id;
    let match_end_url = if config.dathost.match_end_url == None {
        ""
    } else {
        config.dathost.match_end_url.as_ref().unwrap()
    };
    let start_match_url = String::from("https://dathost.net/api/0.1/matches");
    println!("match_end_webhook_url:'{}'", &match_end_url);
    println!("game_server_id:'{}'", &server_id);
    let mut auth_str = String::new();
    if let Some(scrim_api_config) = config.scrimbot_api_config.clone() {
        auth_str.push_str(&scrim_api_config.scrimbot_api_user.as_ref().unwrap());
        auth_str.push(':');
        auth_str.push_str(&scrim_api_config.scrimbot_api_password.as_ref().unwrap());
        let base64 = base64::encode(&auth_str);
        let mut auth_str = String::from("Basic ");
        auth_str.push_str(&base64);
        println!("webhook_authorization_header: '{}'", auth_str);
    }
    let resp = client
        .post(&start_match_url)
        .form(&[
            ("game_server_id", &server_id),
            ("team1_steam_ids", &&team_t),
            ("team2_steam_ids", &&team_ct),
            ("enable_pause", &&String::from("true")),
            ("enable_tech_pause", &&String::from("true")),
            ("webhook_authorization_header", &&auth_str),
            ("match_end_webhook_url", &&match_end_url.to_string()),
        ])
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
        .unwrap();
    println!("Start match response code - {}", &resp.status());

    if !resp.status().is_success() {
        msg.edit(context.clone(), |m| {
            m.content(&format!(
                "Server failed to start, match POST response code: {}",
                &resp.status().as_str()
            ))
        })
        .await?;
    }
    let server_info_url = format!(
        "https://dathost.net/api/0.1/game-servers/{}",
        config.dathost.server_id
    );
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    let server = client
        .get(&server_info_url)
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
        .unwrap()
        .json::<ServerInfoResponse>()
        .await?;
    let client = Client::new();
    let host_name = if server.custom_domain.is_some() {
        server.custom_domain.unwrap()
    } else {
        server.ip
    };
    let game_url = format!("{}:{}", host_name, server.ports.game);
    let gotv_url = format!("{}:{}", host_name, server.ports.gotv);
    let game_link = format!("steam://connect/{}", &game_url);
    let gotv_link = format!("steam://connect/{}", &gotv_url);
    let t_url = client
        .get("https://tinyurl.com/api-create.php")
        .query(&[("url", &game_link)])
        .send()
        .await?
        .text()
        .await?;
    let t_gotv_url = client
        .get("https://tinyurl.com/api-create.php")
        .query(&[("url", &gotv_link)])
        .send()
        .await?
        .text()
        .await?;
    let team_names = context.data().team_names.lock().await.clone();
    let eos = MessageBuilder::new()
        .push_line(list_teams(&draft, &team_names))
        .push_line(format!("Map: `{}`", &draft.selected_map))
        .build();
    msg.edit(context.clone(), |m| {
        m.content(eos).components(|c| {
            c.add_action_row(create_server_conn_button_row(&t_url, &t_gotv_url, true))
        })
    })
    .await?;

    let draft = context.data().draft.lock().await.clone();
    let guild = context.partial_guild().await.unwrap();
    if let Some(team_a_channel_id) = config.discord.team_a_channel_id {
        for user in &draft.team_a {
            if let Err(why) = guild
                .move_member(&context, user.id, team_a_channel_id)
                .await
            {
                println!("Cannot move user: {:?}", why);
            }
        }
    }
    if let Some(team_b_channel_id) = config.discord.team_b_channel_id {
        for user in &draft.team_b {
            if let Err(why) = guild
                .move_member(&context, user.id, team_b_channel_id)
                .await
            {
                println!("Cannot move user: {:?}", why);
            }
        }
    }
    let team_names = context.data().team_names.lock().await.clone();
    let send_command_url = format!(
        "https://dathost.net/api/0.1/game-servers/{}/console",
        &config.dathost.server_id
    );
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    let default_team_a_name = &format!("Team {}", &draft.captain_a.as_ref().unwrap().name);
    let default_team_b_name = &format!("Team {}", &draft.captain_b.as_ref().unwrap().name);
    let team_a_name = team_names
        .get(draft.captain_a.as_ref().unwrap().id.as_u64())
        .unwrap_or(default_team_a_name);
    let team_b_name = team_names
        .get(draft.captain_b.as_ref().unwrap().id.as_u64())
        .unwrap_or(default_team_b_name);
    let team_one_command;
    let team_two_command;
    if draft.team_b_start_side == "ct" {
        team_one_command = format!("mp_teamname_1 {}", &team_b_name);
        team_two_command = format!("mp_teamname_2 {}", &team_a_name);
    } else {
        team_one_command = format!("mp_teamname_1 {}", &team_a_name);
        team_two_command = format!("mp_teamname_2 {}", &team_b_name);
    }
    if let Err(resp) = client
        .post(&send_command_url)
        .form(&[("line", &team_one_command)])
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
    {
        eprintln!("Error setting team name 1: {:?}", resp);
    }
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    if let Err(resp) = client
        .post(&send_command_url)
        .form(&[("line", &team_two_command)])
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
    {
        eprintln!("Error setting team name 2: {:?}", resp);
    }
    // reset to queue state
    {
        let mut user_queue = context.data().user_queue.lock().await;
        user_queue.clear();
    }
    {
        let mut ready_queue = context.data().user_queue.lock().await;
        ready_queue.clear();
    }
    {
        let mut state = context.data().state.lock().await;
        *state = State::Queue;
    }
    {
        let mut queue_msgs = context.data().queue_messages.lock().await;
        queue_msgs.clear();
    }
    reset_draft(context).await?;

    let mut cib = msg
        .clone()
        .into_message()
        .await?
        .await_component_interactions(context)
        .timeout(Duration::from_secs(60 * 5))
        .build();
    loop {
        let opt = cib.next().await;
        match opt {
            Some(mci) => {
                mci.create_interaction_response(context, |r| {
                    r.kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|d| {
                            d.ephemeral(true).content(format!(
                                "Console: ||`connect {}`||\nGOTV: ||`connect {}`||",
                                &game_url, &gotv_url
                            ))
                        })
                })
                .await?;
            }
            None => {
                // remove console cmds interaction on timeout
                msg.into_message()
                    .await?
                    .edit(context, |m| {
                        m.components(|c| {
                            c.add_action_row(create_server_conn_button_row(
                                &t_url,
                                &t_gotv_url,
                                false,
                            ))
                        })
                    })
                    .await?;
                break;
            }
        }
    }
    Ok(())
}

pub fn create_server_conn_button_row(
    url: &String,
    gotv_url: &String,
    show_cmds: bool,
) -> CreateActionRow {
    let mut ar = CreateActionRow::default();
    let mut conn_button = CreateButton::default();
    conn_button.label("Connect");
    conn_button.style(ButtonStyle::Link);
    conn_button.emoji(ReactionType::Unicode("▶".parse().unwrap()));
    conn_button.url(&url);
    ar.add_button(conn_button);
    if show_cmds {
        let mut console_button = CreateButton::default();
        console_button.custom_id("console");
        console_button.label("Console Cmds");
        console_button.style(ButtonStyle::Secondary);
        console_button.emoji(ReactionType::Unicode("🧾".parse().unwrap()));
        ar.add_button(console_button);
    }
    let mut gotv_button = CreateButton::default();
    gotv_button.label("GOTV");
    gotv_button.style(ButtonStyle::Link);
    gotv_button.emoji(ReactionType::Unicode("📺".parse().unwrap()));
    gotv_button.url(gotv_url);
    ar.add_button(gotv_button);
    ar
}
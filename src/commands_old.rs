use std::collections::HashMap;
use std::time::Duration;

use async_std::task;
use rand::Rng;
use regex::Regex;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serenity::client::Context;
use serenity::model::channel::{Message, ReactionType};
use serenity::model::guild::Guild;
use serenity::model::id::EmojiId;
use serenity::model::user::User;
use serenity::utils::MessageBuilder;

use crate::utils::{
    admin_check, convert_steamid_to_64, format_stats, get_maps, list_teams, list_unpicked,
    populate_unicode_emojis, send_simple_msg, send_simple_tagged_msg, write_to_file,
};
struct ReactionResult {
    count: u64,
    map: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct Stats {
    pub steamId: String,
    pub totalKills: f64,
    pub totalDeaths: f64,
    pub totalAssists: f64,
    pub kdRatio: f64,
    pub map: String,
    pub hs: f64,
    pub rws: f64,
    pub adr: f64,
    pub rating: f64,
    pub playCount: i64,
    pub winPercentage: f64,
}

pub(crate) async fn handle_ready_list(context: Context, msg: Message) {
    let data = context.data.write().await;
    let ready_queue: &Vec<User> = data.get::<ReadyQueue>().unwrap();
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    let user_name: String = user_queue
        .iter()
        .filter(|user| !ready_queue.contains(user))
        .map(|user| format!("\n- @{}", user.name))
        .collect();
    let response = MessageBuilder::new()
        .push("Players that are not `.ready`:")
        .push(user_name)
        .build();

    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_start(context: Context, msg: Message) {
    let admin_check = admin_check(&context, &msg, true).await;
    if !admin_check {
        return;
    }
    {
        let mut data = context.data.write().await;
        let bot_state: &StateContainer = data.get::<BotState>().unwrap();
        if bot_state.state != State::Queue {
            send_simple_tagged_msg(
                &context,
                &msg,
                " `.start` command has already been entered",
                &msg.author,
            )
            .await;
            return;
        }
        let user_queue: &mut Vec<User> = data.get_mut::<UserQueue>().unwrap();
        if !user_queue.contains(&msg.author) && !admin_check {
            send_simple_tagged_msg(
                &context,
                &msg,
                " non-admin users that are not in the queue cannot start the match",
                &msg.author,
            )
            .await;
            return;
        }
        if user_queue.len() != 10 {
            let response = MessageBuilder::new()
                .mention(&msg.author)
                .push(" the queue is not full yet")
                .build();
            if let Err(why) = msg.channel_id.say(&context.http, &response).await {
                eprintln!("Error sending message: {:?}", why);
            }
            return;
        }
        let user_queue_mention: String = user_queue
            .iter()
            .map(|user| format!("- <@{}>\n", user.id))
            .collect();
        let response = MessageBuilder::new()
            .push(user_queue_mention)
            .push_bold_line("Scrim setup is starting...")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            eprintln!("Error sending message: {:?}", why);
        }
        let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
        bot_state.state = State::MapPick;
    }
    let maps: Vec<String> = get_maps(&context).await;
    let mut unicode_to_maps: HashMap<String, String> = HashMap::new();
    let a_to_z = ('a'..'z').collect::<Vec<_>>();
    let unicode_emoji_map = populate_unicode_emojis().await;
    for (i, map) in maps.iter().enumerate() {
        unicode_to_maps.insert(
            String::from(unicode_emoji_map.get(&a_to_z[i]).unwrap()),
            String::from(map),
        );
    }
    let emoji_suffixes = a_to_z[..maps.len()].to_vec();
    let vote_text: String = emoji_suffixes
        .iter()
        .enumerate()
        .map(|(i, c)| format!(":regional_indicator_{}: `{}`\n", c, &maps[i]))
        .collect();
    let response = MessageBuilder::new()
        .push_bold_line("Map Vote:")
        .push(vote_text)
        .build();
    let vote_msg = msg.channel_id.say(&context.http, &response).await.unwrap();
    for c in emoji_suffixes {
        vote_msg
            .react(
                &context.http,
                ReactionType::Unicode(String::from(unicode_emoji_map.get(&c).unwrap())),
            )
            .await
            .unwrap();
    }
    task::sleep(Duration::from_secs(50)).await;
    let response = MessageBuilder::new()
        .push("Voting will end in 10 seconds")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }
    task::sleep(Duration::from_secs(10)).await;
    let updated_vote_msg = vote_msg
        .channel_id
        .message(&context.http, vote_msg.id)
        .await
        .unwrap();
    let mut results: Vec<ReactionResult> = Vec::new();
    for reaction in updated_vote_msg.reactions {
        let react_as_map: Option<&String> =
            unicode_to_maps.get(reaction.reaction_type.to_string().as_str());
        if react_as_map != None {
            let map = String::from(react_as_map.unwrap());
            results.push(ReactionResult {
                count: reaction.count,
                map,
            });
        }
    }
    let max_count = results
        .iter()
        .max_by(|x, y| x.count.cmp(&y.count))
        .unwrap()
        .count;
    let final_results: Vec<ReactionResult> = results
        .into_iter()
        .filter(|m| m.count == max_count)
        .collect();
    let mut selected_map = String::from("");
    if final_results.len() > 1 {
        let map = &final_results
            .get(rand::thread_rng().gen_range(0, final_results.len()))
            .unwrap()
            .map;
        let response = MessageBuilder::new()
            .push("Maps were tied, `")
            .push(&map)
            .push("` was selected at random")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            eprintln!("Error sending message: {:?}", why);
        }
        selected_map.push_str(map);
    } else {
        let map = &final_results[0].map;
        let response = MessageBuilder::new()
            .push("Map vote has concluded. `")
            .push(&map)
            .push("` will be played")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            eprintln!("Error sending message: {:?}", why);
        }
        selected_map.push_str(map);
    }
    let mut data = context.data.write().await;
    let config: &Config = data.get::<Config>().unwrap();
    let client = reqwest::Client::new();
    let dathost_username = &config.dathost.username;
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    let update_map_url = format!(
        "https://dathost.net/api/0.1/game-servers/{}",
        &config.server.id
    );
    let resp = client
        .put(&update_map_url)
        .form(&[("csgo_settings.mapgroup_start_map", &selected_map)])
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
        .unwrap();
    println!("Change map response - {:#?}", resp.status());
    let mut bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    bot_state.state = State::DraftTypePick;
    let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
    draft.captain_a = None;
    draft.captain_b = None;
    draft.team_a = Vec::new();
    draft.team_b = Vec::new();
    send_simple_msg(
        &context,
        &msg,
        "Map vote complete. Select draft type using `.autodraft` or `.manualdraft`",
    )
    .await;
}

pub(crate) async fn handle_auto_draft(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let user_queue: &Vec<User> = &data.get::<UserQueue>().unwrap().clone();
    let steam_id_cache: &HashMap<u64, String> = data.get::<SteamIdCache>().unwrap();
    let mut user_queue_steamids: HashMap<u64, String> = HashMap::new();
    let mut user_queue_user_ids: HashMap<String, u64> = HashMap::new();
    for user in user_queue {
        let mut steamid = steam_id_cache.get(user.id.as_u64()).unwrap().to_string();
        steamid = steamid.replacen("STEAM_0", "STEAM_1", 1);
        user_queue_steamids.insert(*user.id.as_u64(), steamid.clone());
        user_queue_user_ids.insert(steamid.clone(), *user.id.as_u64());
    }
    let steamids: String = user_queue_steamids
        .into_values()
        .map(|s| format!("{},", s))
        .collect();

    let config: &Config = data.get::<Config>().unwrap();
    if config
        .scrimbot_api_config
        .clone()
        .unwrap()
        .scrimbot_api_user
        == None
        || config
            .scrimbot_api_config
            .clone()
            .unwrap()
            .scrimbot_api_password
            == None
    {
        send_simple_tagged_msg(&context, &msg, " sorry, the scrimbot-api user/password has not been configured. This option is unavailable.", &msg.author).await;
        return;
    }
    if let Some(scrimbot_api_url) = &config.scrimbot_api_config.clone().unwrap().scrimbot_api_url {
        let mut headers = header::HeaderMap::new();
        let mut auth_str = config
            .scrimbot_api_config
            .clone()
            .unwrap()
            .scrimbot_api_user
            .clone()
            .unwrap();
        auth_str.push(':');
        auth_str.push_str(
            &*config
                .scrimbot_api_config
                .clone()
                .unwrap()
                .scrimbot_api_password
                .clone()
                .unwrap(),
        );
        let base64 = base64::encode(auth_str);
        let mut auth_str = String::from("Basic ");
        auth_str.push_str(&base64);
        headers.insert("Authorization", auth_str.parse().unwrap());
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();
        let resp = client
            .get(&format!("{}/api/stats", scrimbot_api_url))
            .query(&[("steamids", &steamids), ("option", &"players".to_string())])
            .send()
            .await
            .unwrap();
        if resp.status() != 200 {
            eprintln!(
                "{}",
                format!(
                    "HTTP error on /api/stats with following params: steamids: {}, option: players",
                    &steamids
                )
            );
            return;
        }
        let content = resp.text().await.unwrap();
        let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
        if stats.is_empty() {
            send_simple_tagged_msg(
                &context,
                &msg,
                " sorry, no statistics found for any players, please use another option",
                &msg.author,
            )
            .await;
            return;
        }
        if stats.len() < 2 {
            send_simple_tagged_msg(
                &context,
                &msg,
                " sorry, unable to find stats for at least 2 players. Please use another option",
                &msg.author,
            )
            .await;
            return;
        }
        let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
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
        if draft.team_a.len() != 5 || draft.team_b.len() != 5 {
            let mut bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
            bot_state.state = State::Draft;
            let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
            send_simple_msg(&context, &msg, " unable to find stats for all players. Continue draft and pick the remaining players manually.").await;
            let response = MessageBuilder::new()
                .push("It is ")
                .mention(&draft.current_picker.clone().unwrap())
                .push(" turn to `.pick @<user>`")
                .build();
            if let Err(why) = msg.channel_id.say(&context.http, &response).await {
                eprintln!("Error sending message: {:?}", why);
            }
            let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
            let draft: &Draft = data.get::<Draft>().unwrap();
            let teamname_cache = data.get::<TeamNameCache>().unwrap();
            let team_a_name = teamname_cache
                .get(draft.captain_a.as_ref().unwrap().id.as_u64())
                .unwrap_or(&draft.captain_a.as_ref().unwrap().name);
            let team_b_name = teamname_cache
                .get(draft.captain_b.as_ref().unwrap().id.as_u64())
                .unwrap_or(&draft.captain_b.as_ref().unwrap().name);
            list_unpicked(user_queue, draft, &context, &msg, team_a_name, team_b_name).await;
        } else {
            let mut bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
            bot_state.state = State::SidePick;
            let draft: &Draft = data.get::<Draft>().unwrap();
            let teamname_cache = data.get::<TeamNameCache>().unwrap();
            let team_a_name = teamname_cache
                .get(draft.captain_a.as_ref().unwrap().id.as_u64())
                .unwrap_or(&draft.captain_a.as_ref().unwrap().name);
            let team_b_name = teamname_cache
                .get(draft.captain_b.as_ref().unwrap().id.as_u64())
                .unwrap_or(&draft.captain_b.as_ref().unwrap().name);
            list_teams(draft, &context, &msg, team_a_name, team_b_name).await;
            send_simple_tagged_msg(
                &context,
                &msg,
                " type `.ct` or `.t` to pick a starting side.",
                &draft.captain_b.as_ref().unwrap().clone(),
            )
            .await;
        }
    }
}

pub(crate) async fn handle_manual_draft(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let mut bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    bot_state.state = State::CaptainPick;
    send_simple_msg(&context, &msg, "Manual draft selected. Starting captain pick phase. Two users type `.captain` to start picking teams.").await;
}

pub(crate) async fn handle_captain(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    if bot_state.state != State::CaptainPick {
        send_simple_tagged_msg(
            &context,
            &msg,
            " command ignored, not in the captain pick phase",
            &msg.author,
        )
        .await;
        return;
    }
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    if !user_queue.contains(&msg.author) {
        send_simple_tagged_msg(
            &context,
            &msg,
            " command ignored, you are not in the queue",
            &msg.author,
        )
        .await;
        return;
    }
    let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
    if draft.captain_a != None && &msg.author == draft.captain_a.as_ref().unwrap() {
        send_simple_tagged_msg(&context, &msg, " you're already a captain!", &msg.author).await;
        return;
    }
    if draft.captain_a == None {
        send_simple_tagged_msg(&context, &msg, " is set as captain.", &msg.author).await;
        draft.captain_a = Some(msg.author.clone());
    } else {
        send_simple_tagged_msg(&context, &msg, " is set as captain.", &msg.author).await;
        draft.captain_b = Some(msg.author.clone());
    }
    if draft.captain_a != None && draft.captain_b != None {
        send_simple_msg(&context, &msg, "Randomizing captain pick order...").await;
        // flip a coin, if 1 switch captains
        if rand::thread_rng().gen_range(0, 2) != 0 {
            let captain_a = draft.captain_a.clone();
            let captain_b = draft.captain_b.clone();
            draft.captain_a = captain_b;
            draft.captain_b = captain_a;
        }
        draft.team_a.push(draft.captain_a.clone().unwrap());
        draft.team_b.push(draft.captain_b.clone().unwrap());
        send_simple_tagged_msg(
            &context,
            &msg,
            " is set as the first pick captain (Team A)",
            &draft.captain_a.clone().unwrap(),
        )
        .await;
        send_simple_tagged_msg(
            &context,
            &msg,
            " is set as the second captain (Team B)",
            &draft.captain_b.clone().unwrap(),
        )
        .await;
        draft.current_picker = draft.captain_a.clone();
        let response = MessageBuilder::new()
            .push("Captain pick has concluded. Starting draft phase. ")
            .mention(&draft.current_picker.clone().unwrap())
            .push(" gets first `.pick @<user>`")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            eprintln!("Error sending message: {:?}", why);
        }
        let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
        bot_state.state = State::Draft;
        let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
        let draft: &Draft = data.get::<Draft>().unwrap();
        let teamname_cache = data.get::<TeamNameCache>().unwrap();
        let team_a_name = teamname_cache
            .get(draft.captain_a.as_ref().unwrap().id.as_u64())
            .unwrap_or(&draft.captain_a.as_ref().unwrap().name);
        let team_b_name = teamname_cache
            .get(draft.captain_b.as_ref().unwrap().id.as_u64())
            .unwrap_or(&draft.captain_b.as_ref().unwrap().name);
        list_unpicked(user_queue, draft, &context, &msg, team_a_name, team_b_name).await;
    }
}

pub(crate) async fn handle_pick(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    if bot_state.state != State::Draft {
        send_simple_tagged_msg(
            &context,
            &msg,
            " it is not currently the draft phase",
            &msg.author,
        )
        .await;
        return;
    }
    if msg.mentions.is_empty() {
        send_simple_tagged_msg(
            &context,
            &msg,
            " please mention a discord user in the message",
            &msg.author,
        )
        .await;
        return;
    }
    let picked = msg.mentions[0].clone();
    let user_queue: Vec<User> = data.get::<UserQueue>().unwrap().clone();
    if !user_queue.contains(&picked) {
        send_simple_tagged_msg(
            &context,
            &msg,
            " this user is not in the queue",
            &msg.author,
        )
        .await;
        return;
    }
    let draft = data.get::<Draft>().unwrap();
    let current_picker = draft.current_picker.clone().unwrap();
    if msg.author != *draft.captain_a.as_ref().unwrap()
        && msg.author != *draft.captain_b.as_ref().unwrap()
    {
        send_simple_tagged_msg(&context, &msg, " you are not a captain", &msg.author).await;
        return;
    }
    if current_picker != msg.author {
        send_simple_tagged_msg(&context, &msg, " it is not your turn to pick", &msg.author).await;
        return;
    }
    if msg.mentions.is_empty() {
        send_simple_tagged_msg(
            &context,
            &msg,
            " please mention a discord user in your message.",
            &msg.author,
        )
        .await;
        return;
    }
    if draft.team_a.contains(&picked) || draft.team_b.contains(&picked) {
        send_simple_tagged_msg(
            &context,
            &msg,
            " this player is already on a team",
            &msg.author,
        )
        .await;
        return;
    }

    let teamname_cache = data.get::<TeamNameCache>().unwrap();
    let team_a_name = String::from(
        teamname_cache
            .get(draft.captain_a.as_ref().unwrap().id.as_u64())
            .unwrap_or(&draft.captain_a.as_ref().unwrap().name),
    );
    let team_b_name = String::from(
        teamname_cache
            .get(draft.captain_b.as_ref().unwrap().id.as_u64())
            .unwrap_or(&draft.captain_b.as_ref().unwrap().name),
    );
    let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
    if draft.captain_a.as_ref().unwrap() == &current_picker {
        send_simple_tagged_msg(
            &context,
            &msg,
            &format!(" has been added to Team {}", team_a_name),
            &picked,
        )
        .await;
        draft.team_a.push(picked);
        draft.current_picker = draft.captain_b.clone();
        list_unpicked(
            &user_queue,
            draft,
            &context,
            &msg,
            &team_a_name,
            &team_b_name,
        )
        .await;
    } else {
        send_simple_tagged_msg(
            &context,
            &msg,
            &format!(" has been added to Team {}", team_b_name),
            &picked,
        )
        .await;
        draft.team_b.push(picked);
        draft.current_picker = draft.captain_a.clone();
        list_unpicked(
            &user_queue,
            draft,
            &context,
            &msg,
            &team_a_name,
            &team_b_name,
        )
        .await;
    }
    if draft.team_a.len() == 5 && draft.team_b.len() == 5 {
        let captain_b = draft.captain_b.clone().unwrap();
        let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
        bot_state.state = State::SidePick;
        let sidepick_msg = send_simple_tagged_msg(
            &context,
            &msg,
            " type `.ct` or `.t` to pick a starting side.",
            &captain_b,
        )
        .await;
        let config: &mut Config = data.get_mut::<Config>().unwrap();
        if let Some(msg) = sidepick_msg {
            if let Some(emote_ct_id) = &config.discord.emote_ct_id {
                if let Some(emote_ct_name) = &config.discord.emote_ct_name {
                    if let Err(why) = msg
                        .react(
                            &context.http,
                            ReactionType::Custom {
                                animated: false,
                                id: EmojiId(*emote_ct_id),
                                name: Some(String::from(emote_ct_name)),
                            },
                        )
                        .await
                    {
                        eprintln!("Error reacting with custom emoji: {:?}", why)
                    };
                }
            }
            if let Some(emote_t_id) = &config.discord.emote_t_id {
                if let Some(emote_t_name) = &config.discord.emote_t_name {
                    if let Err(why) = msg
                        .react(
                            &context.http,
                            ReactionType::Custom {
                                animated: false,
                                id: EmojiId(*emote_t_id),
                                name: Some(String::from(emote_t_name)),
                            },
                        )
                        .await
                    {
                        eprintln!("Error reacting with custom emoji: {:?}", why)
                    };
                }
            }
        }
    }
}

pub(crate) async fn handle_ct_option(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    if bot_state.state != State::SidePick {
        send_simple_tagged_msg(
            &context,
            &msg,
            " it is not currently the side pick phase",
            &msg.author,
        )
        .await;
        return;
    }
    let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
    if &msg.author != draft.captain_b.as_ref().unwrap() {
        send_simple_tagged_msg(&context, &msg, " you are not Captain B", &msg.author).await;
        return;
    }
    draft.team_b_start_side = String::from("ct");
    let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    bot_state.state = State::Ready;
    send_simple_msg(&context, &msg, "Setup is completed. Type `.ready` when you are able start playing. This is a final ready check, once all players are `.ready` the server and match will immediately start.").await;
}

pub(crate) async fn handle_t_option(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    if bot_state.state != State::SidePick {
        send_simple_tagged_msg(
            &context,
            &msg,
            " it is not currently the side pick phase",
            &msg.author,
        )
        .await;
        return;
    }
    let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
    if &msg.author != draft.captain_b.as_ref().unwrap() {
        send_simple_tagged_msg(&context, &msg, " you are not Captain B", &msg.author).await;
        return;
    }
    draft.team_b_start_side = String::from("t");
    let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    bot_state.state = State::Ready;
    send_simple_msg(&context, &msg, "Setup is completed. Type `.ready` when you are able start playing. This is a final ready check, once all players are `.ready` the server and match will immediately start.").await;
}

pub(crate) async fn handle_map_list(context: Context, msg: Message) {
    let data = context.data.write().await;
    let maps: &Vec<String> = data.get::<Maps>().unwrap();
    let map_str: String = maps.iter().map(|map| format!("- `{}`\n", map)).collect();
    let response = MessageBuilder::new()
        .push_line("Current map pool:")
        .push(map_str)
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_kick(context: Context, msg: Message) {
    if !admin_check(&context, &msg, true).await {
        return;
    }
    let mut data = context.data.write().await;
    let state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    if state.state != State::Queue {
        send_simple_tagged_msg(
            &context,
            &msg,
            " cannot `.kick` the queue after `.start`, use `.cancel` to start over if needed.",
            &msg.author,
        )
        .await;
        return;
    }
    let user_queue: &mut Vec<User> = data.get_mut::<UserQueue>().unwrap();
    let user = &msg.mentions[0];
    if !user_queue.contains(&user) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" is not in the queue.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            eprintln!("Error sending message: {:?}", why);
        }
        return;
    }
    let index = user_queue.iter().position(|r| r.id == user.id).unwrap();
    user_queue.remove(index);
    let response = MessageBuilder::new()
        .mention(user)
        .push(" has been kicked. Queue size: ")
        .push(user_queue.len().to_string())
        .push("/10")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }
}
pub(crate) async fn handle_ready(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &StateContainer = data.get_mut::<BotState>().unwrap();
    if bot_state.state != State::Ready {
        send_simple_tagged_msg(
            &context,
            &msg,
            " command ignored. The draft has not been completed yet",
            &msg.author,
        )
        .await;
        return;
    }
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    if !user_queue.contains(&msg.author) {
        send_simple_tagged_msg(&context, &msg, " you are not in the queue.", &msg.author).await;
        return;
    }
    let ready_queue: &mut Vec<User> = data.get_mut::<ReadyQueue>().unwrap();
    if ready_queue.contains(&msg.author) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(", you're already `.ready`")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            eprintln!("Error sending message: {:?}", why);
        }
        return;
    }
    ready_queue.push(msg.author.clone());
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" is ready. Players ready: ")
        .push(ready_queue.len().to_string())
        .push("/10")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }

    if ready_queue.len() >= 10 {}

    pub(crate) async fn handle_unready(context: Context, msg: Message) {
        let mut data = context.data.write().await;
        let bot_state: &StateContainer = data.get_mut::<BotState>().unwrap();
        if bot_state.state != State::Ready {
            send_simple_tagged_msg(
                &context,
                &msg,
                " command ignored. The draft has not been completed yet",
                &msg.author,
            )
            .await;
            return;
        }
        let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
        if !user_queue.contains(&msg.author) {
            send_simple_tagged_msg(&context, &msg, " you are not in the queue.", &msg.author).await;
            return;
        }
        let ready_queue: &mut Vec<User> = data.get_mut::<ReadyQueue>().unwrap();
        let index = ready_queue
            .iter()
            .position(|r| r.id == msg.author.id)
            .unwrap();
        ready_queue.remove(index);
        send_simple_tagged_msg(&context, &msg, " is no longer `.ready`.", &msg.author).await;
    }

    pub(crate) async fn handle_cancel(context: Context, msg: Message) {
        if !admin_check(&context, &msg, true).await {
            return;
        }
        let mut data = context.data.write().await;
        let bot_state: &StateContainer = data.get::<BotState>().unwrap();
        if bot_state.state == State::Queue {
            send_simple_tagged_msg(
                &context,
                &msg,
                " command only valid during `.start` process",
                &msg.author,
            )
            .await;
            return;
        }
        if bot_state.state == State::MapPick {
            send_simple_tagged_msg(
                &context,
                &msg,
                " please wait until the map vote has concluded before `.cancel`ing",
                &msg.author,
            )
            .await;
            return;
        }
        let ready_queue: &mut Vec<User> = data.get_mut::<ReadyQueue>().unwrap();
        ready_queue.clear();
        let draft: &mut Draft = data.get_mut::<Draft>().unwrap();
        draft.team_a = vec![];
        draft.team_b = vec![];
        draft.captain_a = None;
        draft.captain_b = None;
        draft.current_picker = None;
        let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
        bot_state.state = State::Queue;
        send_simple_tagged_msg(&context, &msg, " `.start` process cancelled.", &msg.author).await;
    }

    pub(crate) async fn handle_stats(context: Context, msg: Message) {
        let data = context.data.write().await;
        let config: &Config = data.get::<Config>().unwrap();
        if config.scrimbot_api_config.clone().is_none() {
            send_simple_tagged_msg(
                &context,
                &msg,
                " scrimbot-api has not been configured",
                &msg.author,
            )
            .await;
            return;
        }
        if config.scrimbot_api_config.clone().unwrap().scrimbot_api_url == None {
            send_simple_tagged_msg(
                &context,
                &msg,
                " scrimbot-api url has not been configured",
                &msg.author,
            )
            .await;
            return;
        }
        if config
            .scrimbot_api_config
            .clone()
            .unwrap()
            .scrimbot_api_user
            == None
            || config
                .scrimbot_api_config
                .clone()
                .unwrap()
                .scrimbot_api_password
                == None
        {
            send_simple_tagged_msg(
                &context,
                &msg,
                " scrimbot-api user/password has not been configured",
                &msg.author,
            )
            .await;
            return;
        }
        if let Some(scrimbot_api_url) =
            &config.scrimbot_api_config.clone().unwrap().scrimbot_api_url
        {
            let mut headers = header::HeaderMap::new();
            let mut auth_str = config
                .scrimbot_api_config
                .clone()
                .unwrap()
                .scrimbot_api_user
                .unwrap();
            auth_str.push(':');
            auth_str.push_str(
                &*config
                    .scrimbot_api_config
                    .clone()
                    .unwrap()
                    .scrimbot_api_password
                    .unwrap(),
            );
            let base64 = base64::encode(auth_str);
            let mut auth_str = String::from("Basic ");
            auth_str.push_str(&base64);
            headers.insert("Authorization", auth_str.parse().unwrap());
            let client = reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .unwrap();
            let steam_id_cache: &HashMap<u64, String> = data.get::<SteamIdCache>().unwrap();
            if steam_id_cache.get(msg.author.id.as_u64()).is_none() {
                send_simple_tagged_msg(
                    &context,
                    &msg,
                    " cannot find your steamId, please assign one using the `.steamid` command",
                    &msg.author,
                )
                .await;
                return;
            }
            let mut steam_id = steam_id_cache.get(msg.author.id.as_u64()).unwrap().clone();
            steam_id.replace_range(6..7, "1");
            let map_idx_start = &msg.content.find('\"');
            let map_idx_end = &msg.content.rfind('\"');
            let mut map_name = String::new();
            if map_idx_start != &None && map_idx_end != &None {
                map_name =
                    String::from(&msg.content[map_idx_start.unwrap() + 1..map_idx_end.unwrap()])
            }
            let split_content = msg.content.trim().split(' ').collect::<Vec<_>>();
            if split_content.len() < 2
                || (split_content.len() > 1 && split_content[1].starts_with('\"'))
            {
                let resp = client
                    .get(&format!("{}/api/stats", scrimbot_api_url))
                    .query(&[("steamid", &steam_id), ("map", &map_name)])
                    .send()
                    .await
                    .unwrap();
                if resp.status() != 200 {
                    send_simple_tagged_msg(
                        &context,
                        &msg,
                        " sorry, something went wrong retrieving stats",
                        &msg.author,
                    )
                    .await;
                    return;
                }
                let content = resp.text().await.unwrap();
                let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
                if stats.is_empty() {
                    send_simple_tagged_msg(
                        &context,
                        &msg,
                        " sorry, no statistics found",
                        &msg.author,
                    )
                    .await;
                    return;
                }
                let top_ten_str = format_stats(
                    &stats,
                    &context,
                    steam_id_cache,
                    msg.guild_id.unwrap().as_u64(),
                    false,
                )
                .await;
                send_simple_tagged_msg(&context, &msg, &top_ten_str, &msg.author).await;
                return;
            }
            let arg_str: String = String::from(split_content[1]);
            let month_regex = Regex::new("\\dm").unwrap();
            if month_regex.is_match(&arg_str) {
                let resp = client
                    .get(&format!("{}/api/stats", scrimbot_api_url))
                    .query(&[
                        (&"steamid", &steam_id),
                        (&"option", &"range".to_string()),
                        (&"length", &arg_str.get(0..1).unwrap().to_string()),
                        (&"map", &map_name),
                    ])
                    .send()
                    .await
                    .unwrap();
                if resp.status() != 200 {
                    eprintln!("{}", format!("HTTP error on /api/stats with following params: steamid: {}, option: range, length: {}", &steam_id, &arg_str.get(0..1).unwrap().to_string()));
                    return;
                }
                let content = resp.text().await.unwrap();
                let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
                if stats.is_empty() {
                    send_simple_tagged_msg(
                        &context,
                        &msg,
                        " sorry, no statistics found for your discord user (yet!)",
                        &msg.author,
                    )
                    .await;
                    return;
                }
                let top_ten_str = format_stats(
                    &stats,
                    &context,
                    steam_id_cache,
                    msg.guild_id.unwrap().as_u64(),
                    false,
                )
                .await;
                send_simple_tagged_msg(&context, &msg, &top_ten_str, &msg.author).await;
                return;
            }
            if &arg_str == "top10" {
                if split_content.len() > 2 && !split_content[2].starts_with("\"") {
                    let month_regex = Regex::new("\\dm").unwrap();
                    let month_arg = split_content[2];
                    if month_regex.is_match(month_arg) {
                        let resp = client
                            .get(&format!("{}/api/stats", scrimbot_api_url))
                            .query(&[
                                ("steamid", &steam_id),
                                ("option", &"top10".to_string()),
                                ("mapCountLimit", &"3".to_string()),
                                ("length", &month_arg.get(0..1).unwrap().to_string()),
                                (&"map", &map_name),
                            ])
                            .send()
                            .await
                            .unwrap();
                        if resp.status() != 200 {
                            eprintln!("{}", format!("HTTP error on /api/stats with following params: steamid: {}, option: top10", &steam_id));
                            return;
                        }
                        let content = resp.text().await.unwrap();
                        let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
                        if stats.is_empty() {
                            send_simple_tagged_msg(
                                &context,
                                &msg,
                                " sorry, no statistics found",
                                &msg.author,
                            )
                            .await;
                            return;
                        }
                        let top_ten_str = format_stats(
                            &stats,
                            &context,
                            steam_id_cache,
                            msg.guild_id.unwrap().as_u64(),
                            false,
                        )
                        .await;
                        if !map_name.is_empty() {
                            map_name = format!("`{}`", &map_name)
                        }
                        send_simple_tagged_msg(
                            &context,
                            &msg,
                            &format!(
                                " Top 10 - {} Month(s) {}:\n{}",
                                &month_arg, &map_name, &top_ten_str
                            ),
                            &msg.author,
                        )
                        .await;
                    } else {
                        send_simple_tagged_msg(
                        &context,
                        &msg,
                        " month parameter is not properly formatted. Example: `.stats top10 1m`",
                        &msg.author,
                    )
                    .await;
                    }
                    return;
                } else {
                    let resp = client
                        .get(&format!("{}/api/stats", scrimbot_api_url))
                        .query(&[
                            ("option", &"top10".to_string()),
                            ("mapCountLimit", &"3".to_string()),
                            (&"map", &map_name),
                        ])
                        .send()
                        .await
                        .unwrap();
                    if resp.status() != 200 {
                        eprintln!("{}", format!("HTTP error on /api/stats with following params: steamid: {}, option: top10", &steam_id));
                        return;
                    }
                    let content = resp.text().await.unwrap();
                    let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
                    if stats.is_empty() {
                        send_simple_tagged_msg(
                            &context,
                            &msg,
                            " sorry, something went wrong retrieving stats",
                            &msg.author,
                        )
                        .await;
                        return;
                    }
                    let top_ten_str = format_stats(
                        &stats,
                        &context,
                        steam_id_cache,
                        msg.guild_id.unwrap().as_u64(),
                        false,
                    )
                    .await;
                    send_simple_tagged_msg(
                        &context,
                        &msg,
                        &format!(" Top 10 (ADR):\n{}", &top_ten_str),
                        &msg.author,
                    )
                    .await;
                    return;
                }
            }
            if &arg_str == "maps" {
                if split_content.len() > 2 && !split_content[2].starts_with("\"") {
                    let month_regex = Regex::new("\\dm").unwrap();
                    let month_arg = split_content[2];
                    if month_regex.is_match(&month_arg) {
                        let resp = client
                            .get(&format!("{}/api/stats", scrimbot_api_url))
                            .query(&[
                                ("steamid", &steam_id),
                                ("option", &"maps".to_string()),
                                ("length", &month_arg.get(0..1).unwrap().to_string()),
                                ("map", &map_name),
                            ])
                            .send()
                            .await
                            .unwrap();
                        if resp.status() != 200 {
                            eprintln!("HTTP error on /api/stats with following params: steamid: {}, option: top10", &steam_id);
                            return;
                        }
                        let content = resp.text().await.unwrap();
                        let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
                        if stats.is_empty() {
                            send_simple_tagged_msg(
                                &context,
                                &msg,
                                " sorry, something went wrong retrieving stats",
                                &msg.author,
                            )
                            .await;
                            return;
                        }
                        let top_ten_str = format_stats(
                            &stats,
                            &context,
                            steam_id_cache,
                            msg.guild_id.unwrap().as_u64(),
                            true,
                        )
                        .await;
                        send_simple_tagged_msg(
                            &context,
                            &msg,
                            &format!(
                                " Top 10 (per map) - {} Month(s):\n{}",
                                &month_arg, &top_ten_str
                            ),
                            &msg.author,
                        )
                        .await;
                    } else {
                        send_simple_tagged_msg(
                        &context,
                        &msg,
                        " month parameter is not properly formatted. Example: `.stats top10 1m`",
                        &msg.author,
                    )
                    .await;
                    }
                } else {
                    let resp = client
                        .get(&format!("{}/api/stats", scrimbot_api_url))
                        .query(&[
                            ("steamid", &steam_id),
                            ("option", &"maps".to_string()),
                            ("map", &map_name),
                        ])
                        .send()
                        .await
                        .unwrap();
                    if resp.status() != 200 {
                        eprintln!("HTTP error on /api/stats with following params: steamid: {}, option: top10", &steam_id);
                        return;
                    }
                    let content = resp.text().await.unwrap();
                    let stats: Vec<Stats> = serde_json::from_str(&content).unwrap();
                    if stats.is_empty() {
                        send_simple_tagged_msg(
                            &context,
                            &msg,
                            " sorry, something went wrong retrieving stats",
                            &msg.author,
                        )
                        .await;
                        return;
                    }
                    let top_ten_str = format_stats(
                        &stats,
                        &context,
                        steam_id_cache,
                        msg.guild_id.unwrap().as_u64(),
                        true,
                    )
                    .await;
                    send_simple_tagged_msg(
                        &context,
                        &msg,
                        &format!(" Top 10 (per map):\n{}", &top_ten_str),
                        &msg.author,
                    )
                    .await;
                }
            }
        }
    }
}

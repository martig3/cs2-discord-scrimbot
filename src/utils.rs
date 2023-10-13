use std::collections::HashMap;

use anyhow::Result;

use crate::{Context, Draft, ScrimbotApiConfig, State};
use poise::serenity_prelude::{Guild, InteractionResponseType, MessageComponentInteraction, User};
use reqwest::header;
use serde::{Deserialize, Serialize};
use serenity::{http::CacheHttp, utils::MessageBuilder};

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

pub(crate) async fn format_stats(
    stats: &Vec<Stats>,
    context: &Context<'_>,
    steam_id_cache: &HashMap<u64, String>,
    &guild_id: &u64,
    print_map: bool,
) -> Result<String> {
    let mut top_ten_str: String = String::from("");
    top_ten_str.push_str("```md\n");
    if stats.len() == 1 {
        let mut map = String::from(stats[0].map.clone());
        if map != "" {
            map = map.replace("de_", "");
            if map.len() > 12 {
                map = map[0..9].to_string();
                map.push_str("...");
            }
            top_ten_str.push_str(&*format!(
                "Map: {:<12} K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n",
                map
            ));
        } else {
            top_ten_str.push_str(
                "                  K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n",
            );
        }
    } else if !print_map {
        top_ten_str.push_str(
            "     Player       K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n",
        );
    } else {
        top_ten_str.push_str(
            "     Maps         K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n",
        );
    }
    top_ten_str.push_str(
        "-----------------------------------------------------------------------------\n",
    );
    let guild = Guild::get(&context.http(), guild_id).await?;
    let mut count = 0;
    for stat in stats {
        count += 1;
        let user_id: Option<u64> = steam_id_cache.iter().find_map(|(key, val)| {
            if format!("STEAM_1{}", &val[7..]) == stat.steamId {
                Some(*key)
            } else {
                None
            }
        });
        let user_cached: Option<User> = context.cache().unwrap().user(user_id.unwrap_or(0));
        let user: Option<User>;
        if let Some(u) = user_cached {
            user = Some(u)
        } else {
            let member = guild.member(&context.http(), user_id.unwrap_or(0)).await;
            if let Ok(m) = member {
                user = Some(m.user)
            } else {
                user = None
            };
        }
        if let Some(u) = user {
            if !print_map {
                let mut user_name = u.name.clone();
                if user_name.len() > 12 {
                    user_name = user_name[0..9].to_string();
                    user_name.push_str("...");
                }
                top_ten_str.push_str(&format!(
                    "{:>3} @{} {:3.2}  {: >6}   {: >6}   {:3.2}     {:3.1}%    {:3.2}% ({})\n",
                    format!("{}.", count.to_string()),
                    format!("{: <12}", user_name.to_owned()),
                    stat.kdRatio,
                    format!("{:.2}", &stat.adr),
                    format!("{:.2}", &stat.rws),
                    stat.rating,
                    stat.hs,
                    stat.winPercentage,
                    stat.playCount
                ));
            } else {
                let mut map = stat.map.clone();
                map = map.replace("de_", "");
                if map.len() > 12 {
                    map = map[0..9].to_string();
                    map.push_str("...");
                }
                top_ten_str.push_str(&format!(
                    "{:>3}  {} {:3.2}   {: >6}  {: >6}   {:3.2}     {:3.1}%    {:3.2}% ({})\n",
                    format!("{}.", count.to_string()),
                    format!("{: <12}", map.to_owned()),
                    stat.kdRatio,
                    format!("{:.2}", &stat.adr),
                    format!("{:.2}", &stat.rws),
                    stat.rating,
                    stat.hs,
                    stat.winPercentage,
                    stat.playCount
                ))
            }
        } else {
            top_ten_str.push_str(&format!(
                "{:>3} Unknown ({})_\n",
                format!("{}.", count.to_string()),
                stat.steamId,
            ))
        };
    }
    top_ten_str.push_str("```");
    Ok(top_ten_str)
}

pub(crate) fn convert_steamid_to_64(steamid: &String) -> u64 {
    let steamid_split: Vec<&str> = steamid.split(":").collect();
    let y = steamid_split[1].parse::<i64>().unwrap();
    let z = steamid_split[2].parse::<i64>().unwrap();
    let steamid_64 = (z * 2) + y + 76561197960265728;
    return steamid_64 as u64;
}

pub(crate) fn list_teams(draft: &Draft, team_names: &HashMap<u64, String>) -> String {
    let team_a_name = team_names
        .get(draft.captain_a.as_ref().unwrap().id.as_u64())
        .unwrap_or(&draft.captain_a.as_ref().unwrap().name);
    let team_b_name = team_names
        .get(draft.captain_b.as_ref().unwrap().id.as_u64())
        .unwrap_or(&draft.captain_b.as_ref().unwrap().name);
    let team_a: String = draft
        .team_a
        .iter()
        .map(|user| format!("- @{}\n", &user.name))
        .collect();
    let team_b: String = draft
        .team_b
        .iter()
        .map(|user| format!("- @{}\n", &user.name))
        .collect();
    let response = MessageBuilder::new()
        .push_bold_line(format!("Team {}:", team_a_name))
        .push_line(team_a)
        .push_bold_line(format!("Team {}:", team_b_name))
        .push_line(team_b)
        .build();
    response
}

pub(crate) async fn write_to_file(path: String, content: String) {
    let mut error_string = String::from("Error writing to ");
    error_string.push_str(&path);
    std::fs::write(path, content).expect(&error_string);
}
pub(crate) async fn user_in_queue(
    context: &Context<'_>,
    mci: Option<&MessageComponentInteraction>,
) -> Result<bool> {
    let queue = context.data().user_queue.lock().await.clone();
    let uids: Vec<u64> = queue.into_iter().map(|u| u.id.0).collect();
    let member_id = &context.author().id.0;
    if !uids.contains(member_id) {
        if let Some(mci) = mci {
            mci.create_interaction_response(context, |r| {
                r.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|d| {
                        d.ephemeral(true).content("You are not in the queue")
                    })
            })
            .await?;
        } else {
            context
                .send(|m| m.ephemeral(true).content("You are not in the queue"))
                .await?;
        }
        return Ok(false);
    }
    return Ok(true);
}

pub fn get_api_client(config: &ScrimbotApiConfig) -> reqwest::Client {
    let mut headers = header::HeaderMap::new();
    let auth_str = format!("TOKEN {}", &config.scrimbot_api_token);
    headers.insert("Authorization", auth_str.parse().unwrap());
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    client
}
pub async fn reset_draft(context: &Context<'_>) -> Result<()> {
    {
        let mut draft = context.data().draft.lock().await;
        draft.captain_a = None;
        draft.captain_b = None;
        draft.current_picker = None;
        draft.team_a = Vec::new();
        draft.team_b = Vec::new();
        draft.team_b_start_side = String::from("");
        draft.map_votes = HashMap::new();
        draft.selected_map = String::new();
    }
    {
        let mut ready_queue = context.data().ready_queue.lock().await;
        ready_queue.clear();
    }
    {
        let mut state = context.data().state.lock().await;
        *state = State::Queue;
    }
    Ok(())
}

pub async fn clear_queue(context: &Context<'_>) -> Result<()> {
    {
        let mut user_queue = context.data().user_queue.lock().await;
        user_queue.clear();
        write_to_file(
            String::from("data/queue.json"),
            serde_json::to_string(&user_queue.clone()).unwrap(),
        )
        .await;
    }
    {
        let mut queue_messages = context.data().queue_messages.lock().await;
        queue_messages.clear();
        write_to_file(
            String::from("data/queue-messages.json"),
            serde_json::to_string(&queue_messages.clone()).unwrap(),
        )
        .await;
    }
    Ok(())
}

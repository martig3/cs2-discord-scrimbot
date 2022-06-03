use std::collections::HashMap;
use serenity::model::prelude::{Guild, GuildContainer, Message, User};
use serenity::prelude::Context;
use serenity::utils::MessageBuilder;
use crate::{Config, Draft, Maps};
use crate::commands::Stats;

pub(crate) async fn send_simple_msg(context: &Context, msg: &Message, text: &str) {
    let response = MessageBuilder::new()
        .push(text)
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn send_simple_tagged_msg(context: &Context, msg: &Message, text: &str, mentioned: &User) -> Option<Message> {
    let response = MessageBuilder::new()
        .mention(mentioned)
        .push(text)
        .build();
    if let Ok(m) = msg.channel_id.say(&context.http, &response).await {
        Some(m)
    } else {
        eprintln!("Error sending message");
        None
    }
}

pub(crate) async fn admin_check(context: &Context, msg: &Message, print_msg: bool) -> bool {
    let data = context.data.write().await;
    let config: &Config = data.get::<Config>().unwrap();
    let role_name = context.cache.role(msg.guild_id.unwrap(), config.discord.admin_role_id).await.unwrap().name;
    if msg.author.has_role(&context.http, GuildContainer::from(msg.guild_id.unwrap()), config.discord.admin_role_id).await.unwrap_or(false) {
        true
    } else {
        if print_msg {
            let response = MessageBuilder::new()
                .mention(&msg.author)
                .push(" this command requires the '")
                .push(role_name)
                .push("' role.")
                .build();
            if let Err(why) = msg.channel_id.say(&context.http, &response).await {
                eprintln!("Error sending message: {:?}", why);
            }
        }
        false
    }
}

pub(crate) async fn format_stats(stats: &Vec<Stats>, context: &Context, steam_id_cache: &HashMap<u64, String>, &guild_id: &u64, print_map: bool) -> String {
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
            top_ten_str.push_str(&*format!("Map: {:<12} K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n", map));
        } else {
            top_ten_str.push_str("                  K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n");
        }
    } else if !print_map {
        top_ten_str.push_str("     Player       K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n");
    } else {
        top_ten_str.push_str("     Maps         K/D    ADR      RWS     Rating   HS%      Win% (# Games)\n");
    }
    top_ten_str.push_str("-----------------------------------------------------------------------------\n");
    let guild = Guild::get(&context.http, guild_id).await.unwrap();
    let mut count = 0;
    for stat in stats {
        count += 1;
        let user_id: Option<u64> = steam_id_cache.iter()
            .find_map(|(key, val)|
                if format!("STEAM_1{}", &val[7..]) == stat.steamId { Some(*key) } else { None }
            );
        let user_cached: Option<User> = context.cache.user(user_id.unwrap_or(0)).await;
        let user: Option<User>;
        if let Some(u) = user_cached {
            user = Some(u)
        } else {
            let member = guild.member(&context.http, user_id.unwrap_or(0)).await;
            if let Ok(m) = member { user = Some(m.user) } else { user = None };
        }
        if let Some(u) = user {
            if !print_map {
                let mut user_name = u.name.clone();
                if user_name.len() > 12 {
                    user_name = user_name[0..9].to_string();
                    user_name.push_str("...");
                }
                top_ten_str.push_str(&format!("{:>3} @{} {:3.2}  {: >6}   {: >6}   {:3.2}     {:3.1}%    {:3.2}% ({})\n", format!("{}.", count.to_string()), format!("{: <12}", user_name.to_owned()), stat.kdRatio, format!("{:.2}", &stat.adr), format!("{:.2}", &stat.rws), stat.rating, stat.hs, stat.winPercentage, stat.playCount));
            } else {
                let mut map = stat.map.clone();
                map = map.replace("de_", "");
                if map.len() > 12 {
                    map = map[0..9].to_string();
                    map.push_str("...");
                }
                top_ten_str.push_str(&format!("{:>3}  {} {:3.2}   {: >6}  {: >6}   {:3.2}     {:3.1}%    {:3.2}% ({})\n", format!("{}.", count.to_string()), format!("{: <12}", map.to_owned()), stat.kdRatio, format!("{:.2}", &stat.adr), format!("{:.2}", &stat.rws), stat.rating, stat.hs, stat.winPercentage, stat.playCount))
            }
        } else {
            top_ten_str.push_str(&format!("{:>3} @Error!\n", format!("{}.", count.to_string())))
        };
    }
    top_ten_str.push_str("```");
    top_ten_str
}

pub(crate) async fn get_maps(context: &Context) -> Vec<String> {
    let data = context.data.write().await;
    let maps: &Vec<String> = data.get::<Maps>().unwrap();
    let cloned: Vec<String> = maps.clone();
    cloned
}

pub(crate) async fn populate_unicode_emojis() -> HashMap<char, String> {
// I hate this implementation and I deserve to be scolded
// in my defense however, you have to provide unicode emojis to the api
// if Discord's API allowed their shortcuts i.e. ":smile:" instead that would have been more intuitive
    let mut map = HashMap::new();
    map.insert('a', String::from("🇦"));
    map.insert('b', String::from("🇧"));
    map.insert('c', String::from("🇨"));
    map.insert('d', String::from("🇩"));
    map.insert('e', String::from("🇪"));
    map.insert('f', String::from("🇫"));
    map.insert('g', String::from("🇬"));
    map.insert('h', String::from("🇭"));
    map.insert('i', String::from("🇮"));
    map.insert('j', String::from("🇯"));
    map.insert('k', String::from("🇰"));
    map.insert('l', String::from("🇱"));
    map.insert('m', String::from("🇲"));
    map.insert('n', String::from("🇳"));
    map.insert('o', String::from("🇴"));
    map.insert('p', String::from("🇵"));
    map.insert('q', String::from("🇶"));
    map.insert('r', String::from("🇷"));
    map.insert('s', String::from("🇸"));
    map.insert('t', String::from("🇹"));
    map.insert('u', String::from("🇺"));
    map.insert('v', String::from("🇻"));
    map.insert('w', String::from("🇼"));
    map.insert('x', String::from("🇽"));
    map.insert('y', String::from("🇾"));
    map.insert('z', String::from("🇿"));
    map
}

pub(crate) fn convert_steamid_to_64(steamid: &String) -> u64 {
    let steamid_split: Vec<&str> = steamid.split(":").collect();
    let y = steamid_split[1].parse::<i64>().unwrap();
    let z = steamid_split[2].parse::<i64>().unwrap();
    let steamid_64 = (z * 2) + y + 76561197960265728;
    return steamid_64 as u64;
}

pub(crate) async fn list_teams(draft: &Draft, context: &Context, msg: &Message, team_a_name: &String, team_b_name: &String) {
    let team_a: String = draft.team_a
        .iter()
        .map(|user| format!("- @{}\n", &user.name))
        .collect();
    let team_b: String = draft.team_b
        .iter()
        .map(|user| format!("- @{}\n", &user.name))
        .collect();
    let response = MessageBuilder::new()
        .push_bold_line(format!("Team {}:", team_a_name))
        .push_line(team_a)
        .push_bold_line(format!("Team {}:", team_b_name))
        .push_line(team_b)
        .build();

    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }
}


pub(crate) async fn list_unpicked(user_queue: &Vec<User>, draft: &Draft, context: &Context, msg: &Message, team_a_name: &String, team_b_name: &String) {
    let remaining_users: String = user_queue
        .iter()
        .filter(|user| !draft.team_a.contains(user) && !draft.team_b.contains(user))
        .map(|user| format!("- @{}\n", &user.name))
        .collect();
    let team_a: String = draft.team_a
        .iter()
        .map(|user| format!("- @{}\n", &user.name))
        .collect();
    let team_b: String = draft.team_b
        .iter()
        .map(|user| format!("- @{}\n", &user.name))
        .collect();
    let response = MessageBuilder::new()
        .push_bold_line(format!("Team {}:", team_a_name))
        .push_line(team_a)
        .push_bold_line(format!("Team {}:", team_b_name))
        .push_line(team_b)
        .push_bold_line("Remaining players: ")
        .push_line(remaining_users)
        .build();

    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        eprintln!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn write_to_file(path: String, content: String) {
    let mut error_string = String::from("Error writing to ");
    error_string.push_str(&path);
    std::fs::write(path, content)
        .expect(&error_string);
}

use std::collections::HashMap;
use std::time::Duration;

use async_std::task;
use rand::Rng;
use serenity::client::Context;
use serenity::model::channel::{Message, ReactionType};
use serenity::model::user::User;
use serenity::utils::MessageBuilder;

use crate::{BotState, Config, Draft, Maps, ReadyQueue, State, StateContainer, SteamIdCache, UserQueue};
use regex::Regex;

struct ReactionResult {
    count: u64,
    map: String,
}

pub(crate) async fn handle_join(context: &Context, msg: &Message, author: &User) {
    let mut data = context.data.write().await;
    let steam_id_cache: &HashMap<u64, String> = &data.get::<SteamIdCache>().unwrap();
    if !steam_id_cache.contains_key(author.id.as_u64()) {
        let response = MessageBuilder::new()
            .mention(author)
            .push(" steamID not found for your discord user, \
                    please use `.steamid <your steamID>` to assign one. Example: `.steamid STEAM_0:1:12345678` ")
            .push("\nhttps://steamid.io/ is an easy way to find your steamID for your account")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let user_queue: &mut Vec<User> = &mut data.get_mut::<UserQueue>().unwrap();
    if user_queue.contains(&author) {
        let response = MessageBuilder::new()
            .mention(author)
            .push(" is already in the queue.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    user_queue.push(author.clone());
    let response = MessageBuilder::new()
        .mention(author)
        .push(" has been added to the queue. Queue size: ")
        .push(user_queue.len().to_string())
        .push("/10")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_leave(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let user_queue: &mut Vec<User> = data.get_mut::<UserQueue>().unwrap();
    if !user_queue.contains(&msg.author) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" is not in the queue. Type `!join` to join the queue.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let index = user_queue.iter().position(|r| r.id == msg.author.id).unwrap();
    user_queue.remove(index);
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" has left the queue. Queue size: ")
        .push(user_queue.len().to_string())
        .push("/10")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_list(context: Context, msg: Message) {
    let data = context.data.write().await;
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    let mut user_name = String::from("");
    for user in user_queue {
        user_name.push_str("\n- @");
        user_name.push_str(&user.name);
    }
    let queue_len = &user_queue.len();
    let response = MessageBuilder::new()
        .push("Current queue size: ")
        .push(queue_len)
        .push("/10")
        .push(user_name)
        .build();

    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_recover_queue(context: Context, msg: Message) {
    {
        let mut data = context.data.write().await;
        let user_queue: &mut Vec<User> = &mut data.get_mut::<UserQueue>().unwrap();
        user_queue.clear();
    }
    for mention in &msg.mentions {
        handle_join(&context, &msg, &mention).await
    }
}

pub(crate) async fn handle_ready_list(context: Context, msg: Message) {
    let data = context.data.write().await;
    let ready_queue: &Vec<User> = data.get::<ReadyQueue>().unwrap();
    let user_queue: &Vec<User> = data.get::<UserQueue>().unwrap();
    let mut user_name = String::from("");
    for user in user_queue {
        if !ready_queue.contains(user) {
            user_name.push_str("\n- @");
            user_name.push_str(&user.name);
        }
    }
    let response = MessageBuilder::new()
        .push("Players that are not `.ready`:")
        .push(user_name)
        .build();

    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_start(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &StateContainer = data.get::<BotState>().unwrap();
    if bot_state.state != State::Queue {
        send_simple_tagged_msg(&context, &msg, " `.start` command has already been entered", &msg.author).await;
        return;
    }
    let user_queue: &mut Vec<User> = data.get_mut::<UserQueue>().unwrap();
    if !user_queue.contains(&msg.author) {
        send_simple_tagged_msg(&context, &msg, " users that are not in the queue cannot start the match", &msg.author).await;
        return;
    }
    // if user_queue.len() != 10 {
    //     let response = MessageBuilder::new()
    //         .mention(&msg.author)
    //         .push(" the queue is not full yet")
    //         .build();
    //     if let Err(why) = msg.channel_id.say(&context.http, &response).await {
    //         println!("Error sending message: {:?}", why);
    //     }
    //     return;
    // }
    let bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    bot_state.state = State::MapPick;
    let maps: &Vec<String> = &data.get::<Maps>().unwrap();
    let mut unicode_to_maps: HashMap<String, String> = HashMap::new();
    let a_to_z = ('a'..'z').map(|f| f).collect::<Vec<_>>();
    let unicode_emoji_map = populate_unicode_emojis().await;
    for (i, map) in maps.iter().enumerate() {
        unicode_to_maps.insert(String::from(unicode_emoji_map.get(&a_to_z[i]).unwrap()), String::from(map));
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
        vote_msg.react(&context.http, ReactionType::Unicode(String::from(unicode_emoji_map.get(&c).unwrap()))).await.unwrap();
    }
    task::sleep(Duration::from_secs(50)).await;
    let response = MessageBuilder::new()
        .push("Voting will end in 10 seconds")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
    task::sleep(Duration::from_secs(10)).await;
    let updated_vote_msg = vote_msg.channel_id.message(&context.http, vote_msg.id).await.unwrap();
    let mut results: Vec<ReactionResult> = Vec::new();
    for reaction in updated_vote_msg.reactions {
        let map = String::from(unicode_to_maps.get(reaction.reaction_type.to_string().as_str()).unwrap());
        results.push(ReactionResult {
            count: reaction.count,
            map,
        });
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
        let map = &final_results.get(rand::thread_rng().gen_range(0, final_results.len())).unwrap().map;
        let response = MessageBuilder::new()
            .push("Maps were tied, `")
            .push(&map)
            .push("` was selected at random")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
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
            println!("Error sending message: {:?}", why);
        }
        selected_map.push_str(map);
    }
    let config: &Config = data.get::<Config>().unwrap();
    let client = reqwest::Client::new();
    let dathost_username = &config.dathost.username;
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    let mut update_map_url = String::from("https://dathost.net/api/0.1/game-servers/");
    update_map_url.push_str(&config.server.id);
    let resp = client
        .put(&update_map_url)
        .form(&[("csgo_settings.mapgroup_start_map", &selected_map)])
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
        .unwrap();
    println!("Change map response - {:#?}", resp.status());
    let mut bot_state: &mut StateContainer = data.get_mut::<BotState>().unwrap();
    bot_state.state = State::CaptainPick;
    let draft: &mut Draft = &mut data.get_mut::<Draft>().unwrap();
    draft.captain_a = None;
    draft.captain_b = None;
    draft.team_a = Vec::new();
    draft.team_b = Vec::new();
    send_simple_msg(&context, &msg, "Starting captain pick phase. Two users type `.captain` to start picking teams.").await;
}


pub(crate) async fn handle_captain(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &mut StateContainer = &mut data.get_mut::<BotState>().unwrap();
    if bot_state.state != State::CaptainPick {
        send_simple_tagged_msg(&context, &msg, " command ignored, not in the captain pick phase", &msg.author).await;
        return;
    }
    let draft: &mut Draft = &mut data.get_mut::<Draft>().unwrap();
    if &msg.mentions.len() > &2 || &msg.mentions.len() != &0 {
        if msg.mentions.len() > 2 {
            send_simple_tagged_msg(&context, &msg, ", too many users were tagged. There can only be two captains max.", &msg.author).await;
        }
        if msg.mentions.len() != 0 {
            send_simple_tagged_msg(&context, &msg, ", not enough users were tagged. Please tag two users.", &msg.author).await;
        }
        return;
    }
    if msg.mentions.len() == 2 {
        draft.captain_a = Some(msg.mentions[0].clone());
        draft.captain_b = Some(msg.mentions[1].clone());
        draft.team_a.push(draft.captain_a.clone().unwrap());
        draft.team_b.push(draft.captain_b.clone().unwrap());
        send_simple_msg(&context, &msg, "Captains set manually.").await;
    } else {
        if draft.captain_a == None {
            send_simple_tagged_msg(&context, &msg, " is set as the first pick captain (Team A).", &msg.author).await;
            draft.captain_a = Some(msg.author);
            draft.team_a.push(draft.captain_a.clone().unwrap());
        } else {
            send_simple_tagged_msg(&context, &msg, " is set as the second captain (Team B).", &msg.author).await;
            draft.captain_b = Some(msg.author);
            draft.team_b.push(draft.captain_b.clone().unwrap());
        }
    }
    if draft.captain_a != None && draft.captain_b != None {
        draft.current_picker = draft.captain_a.clone();
        let response = MessageBuilder::new()
            .push("Captain pick has concluded. Starting draft phase. ")
            .mention(&draft.current_picker.clone().unwrap())
            .push(" gets first `.pick @<user>`")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        let bot_state: &mut StateContainer = &mut data.get_mut::<BotState>().unwrap();
        bot_state.state = State::Draft;
    }
}

pub(crate) async fn handle_pick(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &mut StateContainer = &mut data.get_mut::<BotState>().unwrap();
    if bot_state.state != State::Draft {
        send_simple_tagged_msg(&context, &msg, " it is not currently the draft phase", &msg.author).await;
        return;
    }
    if msg.mentions.len() == 0 {
        send_simple_tagged_msg(&context, &msg, " please mention a discord user in your message.", &msg.author).await;
        return;
    }
    let picked = msg.mentions[0].clone();
    let user_queue: &Vec<User> = &data.get::<UserQueue>().unwrap().to_vec();
    if !user_queue.contains(&picked) {
        send_simple_tagged_msg(&context, &msg, " this user is not in the queue", &msg.author).await;
        return;
    }
    let draft: &mut Draft = &mut data.get_mut::<Draft>().unwrap();
    if draft.current_picker.clone().unwrap() != msg.author {
        send_simple_tagged_msg(&context, &msg, " it is not your turn to pick", &msg.author).await;
        return;
    }

    if draft.team_a.contains(&draft.current_picker.clone().unwrap()) {
        if !draft.team_a.contains(&picked) || !draft.team_b.contains(&picked) {
            send_simple_tagged_msg(&context, &msg, " has been added to Team A", &picked).await;
            draft.team_a.push(picked);
            draft.current_picker = draft.captain_b.clone();
            list_unpicked(&user_queue, &draft, &context, &msg).await;
        } else {
            send_simple_tagged_msg(&context, &msg, " already is on a team", &picked).await;
        }
    } else {
        if draft.team_b.contains(&draft.current_picker.clone().unwrap()) {
            if !draft.team_a.contains(&picked) || !draft.team_b.contains(&picked) {
                send_simple_tagged_msg(&context, &msg, " has been added to Team B", &picked).await;
                draft.team_b.push(picked);
                draft.current_picker = draft.captain_a.clone();
                list_unpicked(&user_queue, &draft, &context, &msg).await;
            } else {
                send_simple_tagged_msg(&context, &msg, " already is on a team", &picked).await;
            }
        }
    }
    if draft.team_a.len() == 5 && draft.team_b.len() == 5 {
        let bot_state: &mut StateContainer = &mut data.get_mut::<BotState>().unwrap();
        bot_state.state = State::Live;
        send_simple_msg(&context, &msg, "Draft has concluded. Type `.ready` to ready up. Once all players are `.ready` the server will launch.").await;
    }
}

pub(crate) async fn list_unpicked(user_queue: &Vec<User>, draft: &Draft, context: &Context, msg: &Message) {
    let mut user_name = String::from("");
    for user in user_queue {
        if !draft.team_a.contains(user) || !draft.team_b.contains(user) {
            user_name.push_str("\n- @");
            user_name.push_str(&user.name);
        }
    }
    let response = MessageBuilder::new()
        .push("Remaining players: ")
        .push(user_name)
        .build();

    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_steam_id(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let steam_id_cache: &mut HashMap<u64, String> = &mut data.get_mut::<SteamIdCache>().unwrap();
    let steam_id = msg.content.trim().split(" ").take(2).count();
    if steam_id == 1 {
        send_simple_tagged_msg(&context, &msg, " please check the command formatting. There must be a space in between `.steamid` and your steamid. \
        Example: `.steamid STEAM_0:1:12345678`", &msg.author).await;
        return;
    }
    let steam_id_str: String = String::from(msg.content.trim().split(" ").take(2).collect::<Vec<_>>()[1]);
    let steam_id_regex = Regex::new("^STEAM_[0-5]:[01]:\\d+$").unwrap();
    if !steam_id_regex.is_match(&steam_id_str) {
        send_simple_tagged_msg(&context, &msg, " invalid steamid formatting. Example: `.steamid STEAM_0:1:12345678`", &msg.author).await;
        return;
    }
    steam_id_cache.insert(*msg.author.id.as_u64(), String::from(&steam_id_str));
    write_to_file(String::from("steam-ids.json"), serde_json::to_string(steam_id_cache).unwrap()).await;
    let response = MessageBuilder::new()
        .push("Updated steamid for ")
        .mention(&msg.author)
        .push(" to `")
        .push(&steam_id_str)
        .push("`")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_add_map(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let maps: &mut Vec<String> = data.get_mut::<Maps>().unwrap();
    if maps.len() == 26 {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" unable to add map, max amount reached.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let map_name: String = String::from(msg.content.trim().split(" ").take(2).collect::<Vec<_>>()[1]);
    if maps.contains(&map_name) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" unable to add map, already exists.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    maps.push(String::from(&map_name));
    write_to_file(String::from("maps.json"), serde_json::to_string(maps).unwrap()).await;
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" added map: `")
        .push(&map_name)
        .push("`")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_remove_map(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let maps: &mut Vec<String> = data.get_mut::<Maps>().unwrap();
    let map_name: String = String::from(msg.content.trim().split(" ").take(2).collect::<Vec<_>>()[1]);
    if !maps.contains(&map_name) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(" this map doesn't exist in the list.")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
        }
        return;
    }
    let index = maps.iter().position(|m| m == &map_name).unwrap();
    maps.remove(index);
    write_to_file(String::from("maps.json"), serde_json::to_string(maps).unwrap()).await;
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" removed map: `")
        .push(&map_name)
        .push("`")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn handle_unknown(context: Context, msg: Message) {
    let response = MessageBuilder::new()
        .push("Unknown command, type `.help` for list of commands.")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn write_to_file(path: String, content: String) {
    let mut error_string = String::from("Error writing to ");
    error_string.push_str(&path);
    std::fs::write(path, content)
        .expect(&error_string);
}

pub(crate) async fn handle_launch_server(context: &Context, msg: &Message) {
    let data = context.data.write().await;
    let draft: &Draft = &data.get::<Draft>().unwrap();
    let steam_id_cache: &HashMap<u64, String> = &data.get::<SteamIdCache>().unwrap();
    let mut team_a_steam_ids: String = draft.team_a
        .iter()
        .map(|u| steam_id_cache.get(u.id.as_u64()).unwrap().replacen('0', "1", 1))
        .map(|s| format!("{},", s))
        .collect();
    team_a_steam_ids.remove(team_a_steam_ids.len());
    let mut team_b_steam_ids: String = draft.team_b
        .iter()
        .map(|u| steam_id_cache.get(u.id.as_u64()).unwrap().replacen('0', "1", 1))
        .map(|s| format!("{},", s))
        .collect();
    team_b_steam_ids.remove(team_b_steam_ids.len());
    println!("Team A steamids: '{}'", &team_a_steam_ids);
    println!("Team B steamids: '{}'", &team_b_steam_ids);
    let response = MessageBuilder::new()
        .mention(&msg.author)
        .push(" server is starting...")
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }

    let config: &Config = data.get::<Config>().unwrap();
    let client = reqwest::Client::new();
    let dathost_username = &config.dathost.username;
    let dathost_password: Option<String> = Some(String::from(&config.dathost.password));
    let server_id = &config.server.id;

    let start_match_url = String::from("https://dathost.net/api/0.1/matches");

    let resp = client
        .put(&start_match_url)
        .form(&[("game_server_id", &server_id),
            ("team1_steam_ids", &&team_a_steam_ids),
            ("team2_steam_ids", &&team_b_steam_ids)])
        .basic_auth(&dathost_username, dathost_password)
        .send()
        .await
        .unwrap();
    println!("Start match response - {:#?}", resp);

    let mut steam_web_url = String::from("steam://connect/");
    steam_web_url.push_str(&config.server.url);
    send_simple_msg(&context, &msg, &format!("Server has started. Open the following link to connect {}", steam_web_url)).await;
}

pub(crate) async fn handle_ready(context: Context, msg: Message) {
    let mut data = context.data.write().await;
    let bot_state: &StateContainer = data.get::<BotState>().unwrap();
    if bot_state.state != State::Ready {
        send_simple_tagged_msg(&context, &msg, " command ignored. The draft has not been completed yet", &msg.author).await;
        return;
    }
    let ready_queue: &mut Vec<User> = &mut data.get_mut::<ReadyQueue>().unwrap();
    if ready_queue.contains(&msg.author) {
        let response = MessageBuilder::new()
            .mention(&msg.author)
            .push(", you're already `.ready`")
            .build();
        if let Err(why) = msg.channel_id.say(&context.http, &response).await {
            println!("Error sending message: {:?}", why);
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
        println!("Error sending message: {:?}", why);
    }

    if ready_queue.len() == 10 {
        handle_launch_server(&context, &msg).await;
        let draft: &Draft = &data.get::<Draft>().unwrap();
        let config: &Config = &data.get::<Config>().unwrap();
        for user in &draft.team_a {
            if let Some(guild) = &msg.guild(&context.cache).await {
                if let Err(why) = guild.move_member(&context.http, user.id, config.discord.team_a_channel_id).await {
                    println!("Cannot move user: {:?}", why);
                }
            }
        }
        for user in &draft.team_b {
            if let Some(guild) = &msg.guild(&context.cache).await {
                if let Err(why) = guild.move_member(&context.http, user.id, config.discord.team_b_channel_id).await {
                    println!("Cannot move user: {:?}", why);
                }
            }
        }
        let user_queue: &mut Vec<User> = data.get_mut::<UserQueue>().unwrap();
        user_queue.clear();
        let ready_queue: &mut Vec<User> = data.get_mut::<ReadyQueue>().unwrap();
        ready_queue.clear();
    }
}

pub(crate) async fn send_simple_msg(context: &Context, msg: &Message, text: &str) {
    let response = MessageBuilder::new()
        .push(text)
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn send_simple_tagged_msg(context: &Context, msg: &Message, text: &str, mentioned: &User) {
    let response = MessageBuilder::new()
        .mention(mentioned)
        .push(text)
        .build();
    if let Err(why) = msg.channel_id.say(&context.http, &response).await {
        println!("Error sending message: {:?}", why);
    }
}

pub(crate) async fn populate_unicode_emojis() -> HashMap<char, String> {
    // I hate this implementation and I deserve to be scolded
    // in my defense however, you have to provide unicode emojis to the api
    // if Discord allowed their shortcuts i.e. ":smile:" instead that would have been more intuitive
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
    return map;
}

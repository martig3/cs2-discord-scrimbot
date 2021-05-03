# csgo-discord-scrimbot

Discord bot for managing, automating & organizing 10 man scrims in CSGO

## Features

- Manages a 10 person queue
- Map Vote
- Captain pick & player draft
- Starting side pick
- Automatically starts CSGO server & prints out connection info
- Custom team names
- Autoclear queue
- Auto assign discord role to user on queue join  
- Integration with [scrimbot-api](https://github.com/Martig3/scrimbot-api) stats (experimental)

### Dedicated Server Host Support

Supported server hosting platforms are:

- DatHost.net

## Setup

No release binaries yet; so clone the repo, create a `config.yaml` file in the root folder (see example below) and run
using standard `cargo run`

**Note:** Make sure to only allow the bot to listen/read messages in one channel only.

```yaml
autoclear_hour: <0-24> -- optional
scrimbot_api_url: <scrimbot api url> -- optional, experimental
post_setup_msg: GLHF! -- optional
server:
  id: <your dathost server id>
  url: <your dathost server url>
dathost:
  username: <your dathost username/email>
  password: <your dathost password>
  match_end_url: <your match end url>
discord:
  token: <discord bot token>
  admin_role_id: <a discord role id for admins>
  privileged_role_ids: [ string list of discord role ids ]  -- optional
  team_a_channel_id: <a discord text channel id>  -- optional
  team_b_channel_id: <a discord text channel id>  -- optional
  emote_ct_id: <a custom discord emote id>  -- optional
  emote_t_id: <a custom discord emote id> -- optional
  emote_ct_name: <a custom discord emote name> -- optional
  emote_t_name: <a custom discord emote name> -- optional
  assign_role_id: <a dicord role id to assign for user on queue join> -- optional

```

## Commands

`.join` - Join the queue, add an optional message in quotes (max 50 characters) i.e. `.join "available at 9pm"`

`.leave` - Leave the queue

`.list` - List all users in the queue

`.steamid` - Set your steamID i.e. `.steamid STEAM_0:1:12345678`

`.maps` - Lists all maps in available for play

`.stats` - _Experimental Feature_: Lists all available statistics for user. Add ` Xm` to display past X months where X
is a single digit integer. Add `.top10` to display top 10 ranking with an optional `.top10 Xm` month filter.

`.teamname` - Sets a custom team name when you are a captain i.e. `.teamname TeamName`

_These are commands used during the `.start` process:_

`.captain` - Add yourself as a captain.

`.pick` - If you are a captain, this is used to pick a player

`.ready` - After the draft phase is completed, use this to ready up

`.unready` - After the draft phase is completed, use this to cancel your `.ready` status

`.readylist` - Lists players not readied up

**Admin Commands**

`.start` - Start the match setup process

`.kick` - Kick a player by mentioning them i.e. `.kick @user`

`.addmap` - Add a map to the map vote i.e. `.addmap de_dust2` _Note: map must be present on the server or the server
will not start._

`.removemap` - Remove a map from the map vote i.e. `.removemap de_dust2`

`.recoverqueue` - Manually set a queue, tag all users to add after the command

`.clear` - Clear the queue

`.cancel` - Cancels `.start` process

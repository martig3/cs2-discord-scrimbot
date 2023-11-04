# cs2-discord-scrimbot

Discord bot for managing, automating & organizing 10 man scrims in Counter-Strike 2

## Features

- Manages a 10 person queue
- Map Vote
- Captain pick & player draft
- Starting side pick
- Automatically starts CS2 server & prints out connection info
- Custom team names
- Autoclear queue
- Auto assign discord role to user on queue join
- Integration with [scrimbot-api](https://github.com/Martig3/scrimbot-api) stats (experimental)

### Dedicated Server Host Support

Supported server hosting platforms are:

- DatHost.net

## Setup

Run your platform executable with the following `config/config.yaml` file:

```yaml
autoclear_hour: <0-24> -- optional
post_setup_msg: GLHF! -- optional
dathost:
  username: dathost username
  password: dathost password
  server_id: dathost server id
  match_end_url: match-end webhook url
discord:
  token: <discord bot token>
  admin_role_id: <a discord role id for admins>
  privileged_role_ids: [ string list of discord role ids ]  -- optional
  team_a_channel_id: <a discord text channel id>  -- optional
  team_b_channel_id: <a discord text channel id>  -- optional
  assign_role_id: <a dicord role id to assign for user on queue join> -- optional
scrimbot_api_config:
  scrimbot_api_url: <scrimbot-api url>
  scrimbot_api_token: <scrimbot-api auth token>
```
**Note:** Make sure to only allow the bot to listen/read messages in one channel only via the discord server settings -> integrations options.

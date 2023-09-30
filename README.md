# cs2-discord-scrimbot

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

Run your platform executable with the following `config/config.yaml` file:

```yaml
autoclear_hour: <0-24> -- optional
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
  assign_role_id: <a dicord role id to assign for user on queue join> -- optional
scrimbot_api_config: -- optional, experimental section
  scrimbot_api_url: <scrimbot-api url>
  scrimbot_api_user: <scrimbot-api username>
  scrimbot_api_password: <scrimbot-api password>
```
**Note:** Make sure to only allow the bot to listen/read messages in one channel only via the discord server settings -> integrations options.


`.clear` - Clear the queue

`.cancel` - Cancels `.start` process

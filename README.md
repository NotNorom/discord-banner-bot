# Discord Banner Bot

Invite the bot using this link: [invite](https://discord.com/api/oauth2/authorize?client_id=586680217049759744&permissions=274877975648&scope=applications.commands%20bot).

This bot will change the banner of a discord server every few minutes.
Minimum is 15 minutes.
Maximum is currently 2880 minutes (48h).

This is a work in progress, please message me if you have any questions (my timeone is UTC+1 or +2 during summer):
- Discord: [Bot Support Server](https://discord.gg/MMJFtCtYPP)
- Twitter: [@\_norom\_](https://twitter.com/_norom_)
- Email: noromoron \[at\] gmail \[dot\] com


## Usage
Text in \[\] brackets are mandatory arguments.<br>
Text in \<\> brackets are optional arguments.<br>
When entering the commands in discord, don't actually type any brackets.


### /start
`/start [CHANNEL] <INTERVAL> <START_AT> <MESSAGE_LIMIT>`

Start changing banners every INTERVAL minutes.
The banner will be picked randomly from messages in the CHANNEL.
Note: The CHANNEL does not have to be inside the same server, it's just that the bot needs access to the channel.

Interval range:
- minimum: 15
- maximum: 2880 (48h)

`START_AT` is a RFC 3339 formatted date and time string with a timezone.
An example:  
`2016-05-28 22:25:00+02:00` would translate to: May 28th, 2016 at 10pm and 25 minutes in UTC+2 which is daylight savings time in Europe/Berlin.
Note that the timezone part `+02:00` is _required_.

`MESSAGE_LIMIT` ranges from 0 to 200 with a default of 100.  
It is the maximum number of messages the bot will look back in a channel to look for images.
It is not the limit of images.
A message can contain multiple images!

> [!NOTE]
> _Command can only be run by users with `Manage Server` permission._


### /stop
`/stop`

Stop automatic banner changing.

> [!NOTE]
> _Command can only be run by users with `Manage Server` permission.*_


### /channel
`/channel`

Shows the channel link you are using. In case you forgot :D

> [!NOTE]
> _Command can only be run by users with `Manage Server` permission._


### /current
`/current`

Give you a link to the currently displayed banner.

> [!NOTE]
> _Command can only be run by anyone._



### /servers
`/servers`

Displays in how many servers this bot is currently in. It really is just a vanity command.
If run by the bot owners will display private servers as well.

> [!NOTE]
> _Command can only be run by anyone._


### /help
`/help <COMMAND>`

Display a help message. If `COMMAND` is provided, display help about that command.

> [!NOTE]
> _Command can only be run by anyone._


### /start_for_guild
`/start_for_guild [GUILD_ID] [CHANNEL_ID] <INTERVAL> <START_AT> <MESSAGE_LIMIT>`

Same as `/start` but a server can be specified.
This allows to start the bot for servers without the user being in the server.
This is just for bot owners and intended for debugging purposes.

> [!NOTE]
> _Command can only be run by bot owners._


### /stop_for_guild
`/stop_for_guild [GUILD_ID] `

Same as `/stop` but a server can be specified.
This allows to stop the bot for servers without the user being in the server.
This is just for bot owners and intended for debugging purposes.

> [!NOTE]
> _Command can only be run by bot owners._


## Bot information & permissions

Bot username: `@banner changer #2858`.

This bot needs these permissions to work:
- `Manage Server` for editing the banner
- `Read Messages/ View Channels` for using prefix commands
- `Read Message History` for reading messages in a channel to look for images
- `Send Messages` for using prefix commands (and error responses)
- `Send Messages in Threads` for using prefix commands (and error responses)
- `Add Reactions` for nice visuals... eventually :D (things like: this images is not supported)


The following commands can only be run by users with the `Manage Server` permissions:
- `/start`
- `/stop`
- `/channel`
- `/notification_channel`


## Hosting the bot yourself

- Install Rust. Probably using https://rustup.rs/
- Install and start Redis. Probably using https://redis.io/
- Create a discord bot. Probably using https://discord.com/developers/applications
- Clone the project & compile
- Add discord token into settings.toml
- Run

Default settings.toml:
https://github.com/NotNorom/discord-banner-bot/blob/master/settings.template.toml


### Using Docker
You can use docker and docker-compose to run this bot.
A Dockerfile and a docker-compose file are available.


## Redis layout

`PREFIX` is set in settings.toml and defaults to "dbb".

- `PREFIX:active_schedules` keeps a list of currently active guild schedules.
- `PREFIX:active_schedule:GUILD_ID` is a schedule for a specific guild. It contains the following fields:
  - `guild_id`: The guild_id
  - `channel_id`: The channel_id
  - `interval`: Minutes between banner changes
  - `start_at`: Unix timestamp, when the schedule should start
  - `last_run`: Unix timestamp, when the banner was last changed successfully

If `start_at` is in the future (aka the schedule has not been started yet) then `last_run` will be set to `start_at`.
If `start_at` is ever more in the future than `last_run` then something has gone wrong.

## Credits

This bot is built using
- Poise: https://github.com/serenity-rs/poise
- Serenity: https://github.com/serenity-rs/serenity
- Imgurs: https://github.com/M3DZIK/imgurs (back when I was still using imgur)
- Tokio: https://github.com/tokio-rs/tokio/
- Fred: https://github.com/aembke/fred.rs

Amazing libraries, highly recommend \<3

## An example on how to calculate time between schedule runs

`now = 1000`

user's calling:
`/start start_at=1069 interval=150`

insert into database:
`start_at=1069 interval=150 last_run=1069`

bot gets turned off, gets started 100 time units later. `now = 1100`

delay calculation should be:

```
if start_at < now; then
    delay = interval - (now - start_at) % interval
if start_at >= now; then
    delay = start_at - now
```

so at insertion times:

```
start_at=1069 > now=1000; then
    delay=69 = 1069 - 1000
```

and after the bot restart:

```
start_at=1069 < now=1100; then
    delay=119 = interval=100 - (1100 - 1069) % 100
```

## Legal for my hosted bot ( Last Update: 12.12.2024 )

By inviting the bot to your server, you agree to the privacy policy and terms of services as stated below.


### Privacy Policy


#### 1. The following data points are collected from users:

- Guild ids
- Text channel ids
- Message ids
- Message content in the text channel selected


#### 2. Why I need this information

I need the information described above to provide the basic services we offer to you.

I need the guild id to associate other data with the correct guild.
I need the text channel ids to know in which channel to look for content for.
I need the message ids for debugging purposes.
I need the message content to search for media that can be used as a guild banner.


#### 3. How we use your information

We use the information we collect in the following ways:

- Provide, operate, and maintain my discord bot
- Provide all the functionalities that the bot has to offer


#### 4. Other than Discord the company and users of your own bot, who do I share the collected data with

Stored Information is not shared with any third parties.


#### 5. How can users contact you with concerns?

Possible ways of contacting me:
- Discord: [Bot Support Server](https://discord.gg/MMJFtCtYPP)
- Twitter: [@\_norom\_](https://twitter.com/_norom_)
- Email: noromoron \[at\] gmail \[dot\] com


#### 6. If you store data, how can users have that data removed?

Data is automatically removed when the services of the bot is stopped using the `/stop` command.
Debug logs may continue containing information. This information would then be deleted when the bot is restarted.
You can also request deletion of this information via the contact information mentioned above.


### Terms of Service

These terms only apply to the discord application and bot user with this id: `586680217049759744` and when run by me.
These terms do not apply to applications run by 3rd partys, even if the same code is run.

I may change these terms any time I want but I WILL notify you about it before I do.
Breaking any of these terms might result in a ban in using the service.

- When using this application, all of discords Terms of Services apply.
- I reserve the right to terminate and/or deny services to anyone without providing a reason.
- You may not use this bot to inflict harm on others.

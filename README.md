# Discord Banner Bot

Invite the bot using this link: [invite](https://discord.com/api/oauth2/authorize?client_id=586680217049759744&permissions=274877910112&scope=applications.commands%20bot).

This bot will change the banner of a discord server every few minutes.
Minimum is 15 minutes.
Maximum is currently 2880 minutes (48h).

The only supported image hosting service right now is imgur.
I do plan on extending this to others services in the future... eventually... tm.


This is a work in progress, please message me if you have any questions (my timeone is UTC+1 or +2 during summer):
- Discord: [Bot Support Server](https://discord.gg/MMJFtCtYPP)
- Twitter: [@\_norom\_](https://twitter.com/_norom_)
- Email: noromoron \[at\] gmail \[dot\] com


## Usage
Text in \[\] brackets are mandatory arguments.<br>
Text in \<\> brackets are optional arguments.<br>
When entering the commands in discord, don't actually type any brackets.


### /start
`/start [ALBUM] <INTERVAL>`
Start changing banners every INTERVAL minutes.
The banner will be picked randomly from the ALBUM link.

Supported links:
- imgur.com/a/HASH

Interval range:
- minimum: 15
- maximum: 2880 (48h)


### /stop
`/stop`
Stop automatic banner changing.


### /album
`/album`
Shows the album link you are using. In case you forgot :D


### /notification_channel (Does not work yet)
`/notification_channel [CHANNEL]`
When I want to send news or the bot needs to send error messages in your server, this channel will be used.


### /servers
`/servers`
Displays in how many servers this bot is currently in. It really is just a vanity command.


### /help
`/help <COMMAND>`
Display a help message. If COMMAND is provided, display help about that command. 


## Bot information & permissions

Bot username: `@banner changer #2858`.

This bot needs these permissions to work:
- `Manage Server` for editing the banner
- `Read Messages/ View Channels` for using prefix commands
- `Send Messages` for using prefix commands (and error responses)
- `Send Messages in Threads` for using prefix commands (and error responses)
- `Add Reactions` for nice visuals... eventually :D

Right now only server members with the `Manage Server` permission can use the `/start`, `/stop` and `/album` commands.
This might change in the future.


## Credits

This bot is built using
- Poise: https://github.com/kangalioo/poise
- Serenity: https://github.com/serenity-rs/serenity/
- Imgurs: https://github.com/MedzikUser/imgurs

Amazing libraries, highly recommend \<3

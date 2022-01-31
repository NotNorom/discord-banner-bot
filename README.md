# Discord Banner Bot

Invite the bot using this link: [invite](https://discord.com/api/oauth2/authorize?client_id=586680217049759744&permissions=274877910112&scope=applications.commands%20bot).

This bot will change the banner of a discord server every few minutes.
Minimum is 15 minutes.
Maximum is currently 2880 minutes.

This is a work in progress, please message me if you have any questions:
- Discord: norom#1972
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
- `Add Reactions` for nice visuals :D

Right now only server members with the `Manage Server` permission can use the `/start` and `/stop`commands.
This might change in the future.

## Credits

This bot is built using
- Poise: https://github.com/kangalioo/poise
- Serenity: https://github.com/serenity-rs/serenity/

Amazing libraries, highly recommend \<3

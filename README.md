# Wrench
A small little program used to tell if Project Zomboid needs to restart due to a workshop update. It's designed to be used in any server set up.

## How do I use this?
Here's a small guide

## Setup
Wrench requires some initial environment variables to work.

#### Required:
- `STEAM_WEB_KEY:xyz` You need to create a steam web api key and pass it here, you can create [one here](https://steamcommunity.com/dev). This is used to query steam's api for the latest workshop status
- `ACF:path_to_file` You must provide a path to the server's steam workshop acf file. The file sits in steam's workshop folder. The file for Project Zomboid should be called `appworkshop_108600.acf`. This is used to parse what mods are actively used on the server. You should never manually edit this file.

#### Optional:
- ``DISCORD_WEB_HOOK`` You can provide a discord web hook here. The program will send a [small message to said channel with a timestamp to next restart](https://files.vamist.dev/sarcastic-wheat-komododragon/direct.png) (currently 5 mins) and the list of mods that have updated (NOTE: Only past everything after `https://discord.com/api/webhooks`, this was to keep the env less bloated)

### How does it work?
Key things to know:
- It parses the acf file to get a collection of mods
- It queries steam (in one call) with all the mods, and compares the timestamp
- Print's the mod(s) name that need to be updated in terminal (can be captured to be used elsewhere)
- It will return two different types of exit/term codes depending on the result.

### Exit codes
- 0 Means the server should restart, there's an update
- 1 means there's no update/error/crash/ohgodrun

### Why use exit codes
I just wanted something that worked without having to over engineer it, so thats why
Might get changed in the future, but it's good enough for now, and can be paired with bash in useful ways (e.g. && or || based on if its successful or not)

## How do I compile?
- Install rust (https://www.rust-lang.org/tools/install)
- `cargo build --release`. The executable will be placed into a new folder called `target/release`

## TODO:
- Toml config (?)
- Custom 5 min timer
- Allow people to change the webhook pfp & username
- Move acf into its own lib and maybe put it on crates.io
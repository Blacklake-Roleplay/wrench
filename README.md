<p align="center"><img src="https://cdn.discordapp.com/attachments/361255623456849925/978768474727940156/ohno.gif"/></p>


# Wrench
A small little program used to tell if Project Zomboid needs to restart due to a workshop update. It's designed to be used in any server setup.

---

# How do I use this?
Here's a small guide

## Setup
Wrench requires some initial environment variables to work, you can read the most up to date ones by doing `wrench -help.`

#### Required:
- `STEAM_WEB_KEY:xyz` You need to create a steam web api key and pass it here, you can create [one here](https://steamcommunity.com/dev). This is used to query steam's api for the latest workshop status
- `ACF:path_to_file` You must provide a path to the server's steam workshop acf file. The file sits in steam's workshop folder. The file for Project Zomboid should be called `appworkshop_108600.acf`. This is used to parse what mods are actively used on the server. You should never manually edit this file.

#### Optional:
- ``DISCORD_WEB_KEY`` You can provide a discord web hook here. The program will send a [small message to said channel with a timestamp to next restart](https://files.vamist.dev/sarcastic-wheat-komododragon/direct.png) (currently 5 mins) and the list of mods that have updated (NOTE: Only past everything after `https://discord.com/api/webhooks`, this was to keep the env less bloated)
- ``USER_AGENT`` Is required if ``DISCORD_WEB_KEY`` is provided. This is attached to a request when scraping the changelog from steam. The format should include an email, and the programs name (example: `Wrench - Changelog fetcher - your@email.com`).

### How does it work?
Key things to know:
- It parses the acf file to get a collection of mods
- It queries steam (in one call) with all the mods, and compares the timestamp from the acf to the latest version.
- Print's the mod(s) name that need to be updated in terminal (can be captured to be used elsewhere)
- It will return two different types of exit/term codes depending on the result.

### Exit codes
- 0 Means the server should restart, there's an update
- 1 and higher means there's no update/error/crash/oh-god-run

### Why use exit codes
I just wanted something that worked without having to over-engineer it

Might get changed in the future, but it's good enough for now, and can be paired with bash in useful ways (e.g. && or || based on if its successful or not)

---

## Example bash script with wrench integration
This is a simple bash script on how to use wrench, and how you could handle an update.
```
#!/bin/bash

export STEAM_WEB_KEY=abcdef
export ACF=/home/pz/workshop/appworkshop_108600.acf 
export DISCORD_WEB_KEY=1236712/abcade
export USER_AGENT="test@test.com"
 
while true; do 
    sleep 30
    # && means only run on successful exit (e.g. no error code 1 or higher)
    ./wrench && (sleep 5m; tmux kill-session -t pz; ./start_pz)
done;
```

## How do I compile?
- Install rust (https://www.rust-lang.org/tools/install)
- Make sure you have nightly installed (for simd support when fetching changelogs)
- `cargo build --release`. The executable will be placed into a new folder called `target/release`

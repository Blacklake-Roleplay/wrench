extern crate pest;
#[macro_use]
extern crate pest_derive;
mod acf;

use acf::parser;
use acf::parser::AcfValue;

use clap::Parser as ClapParser;
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::ops::Add;
use std::process::exit;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

/// Steam Api mapping
#[derive(Deserialize, Debug)]
struct SteamApiResponse {
    response: Response,
}

#[derive(Deserialize, Debug)]
struct Response {
    publishedfiledetails: Vec<WorkshopContent>,
}

#[derive(Deserialize, Debug)]
struct WorkshopContent {
    title: String,
    publishedfileid: String,
    time_updated: u64,
    preview_url: String,

    // HACK for changelogs, we stuff the changelog here IF discord is enabled
    #[serde(skip_deserializing)]
    changelog: String,
}
///

/// Discord webhook formatting
#[derive(Serialize, Debug)]
struct DiscordWebhook<'a> {
    username: &'a str,
    avatar_url: &'a str,
    content: &'a str,
    embeds: Vec<DiscordEmbeds>,
}

#[derive(Serialize, Debug)]
struct DiscordEmbeds {
    title: String,
    description: String,
    color: String,
    thumbnail: DiscordEmbedThumbnail,
    url: String,
}

#[derive(Serialize, Debug)]
struct DiscordEmbedThumbnail {
    url: String,
}
///

/// Arguments
#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, env("STEAM_WEB_KEY"), help("Steam web api key to do calls with"))]
    steam_key: String,

    #[arg(long, env("DISCORD_WEB_KEY"), requires("user_agent"), help("Pass in a discord webhook to enable discord support, you must also set header_info. Only pass in everything after 'https://discord.com/api/webhooks/'"))]
    discord_key: Option<String>,

    #[arg(long, env("ACF"), help("Points to a workshop .acf file"))]
    acf_file: String,

    #[arg(long, env("USER_AGENT"), help("Required if using discord for changelog support, format it such as `Wrench - email@email.com` so people can reach out if need be"))]
    user_agent: Option<String>,
}
///

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let acf_file = fs::read_to_string(args.acf_file).unwrap();
    let acf: AcfValue =
        parser::file_to_acf(&acf_file).expect("Could not parse the acf file provided");

    let workshop: Option<HashMap<&str, u64>> = match acf {
        AcfValue::Collection(_, body) => Some(parser::extract_workshop(body)),
        _ => None,
    };

    if workshop.is_none() {
        println!("ACF file does not contain any workshop content..");
        exit(1);
    }

    let workshop = workshop.unwrap();

    let all_workshops = {
        let mut total = String::new();
        for (pos, map) in workshop.iter().enumerate() {
            total.push_str(format!("&publishedfileids%5B{}%5D={}", pos, map.0).as_str())
        }

        total
    };

    let url = format!("https://api.steampowered.com/IPublishedFileService/GetDetails/v1/?key={}{}&short_description=true&includeforsaledata=false&includemetadata=false&appid=108600&strip_description_bbcode=true", args.steam_key, all_workshops);

    let client = reqwest::ClientBuilder::new().gzip(true);
    let client = client
        .build()
        .expect("Could not build reqwest client, this is bad");

    let res = client.get(&url).send().await;

    if let Err(why) = res {
        println!("Error sending get request to steams api. {why}, req was {url}");
        exit(1);
    }

    let body = res?.text().await?;

    let res = serde_json::from_str(&body);

    if let Err(why) = res {
        println!("Could not parse steams response to json, reason: {why}, body: ({body})");
        exit(1);
    }

    let res: SteamApiResponse = res?;

    // Iterate in parallel, as we can have quite a few (hundred) mods.
    let mut update_list: Vec<WorkshopContent> = res
        .response
        .publishedfiledetails
        .into_par_iter()
        .filter(|details| {
            let timestamp = workshop.get(details.publishedfileid.as_str());

            if timestamp.is_none() {
                return false;
            }

            let timestamp = timestamp.unwrap();

            if &details.time_updated > timestamp {
                println!("{:?} needs updating!", details.title);
                return true;
            }

            false
        })
        .collect();

    if update_list.is_empty() {
        exit(1);
    }

    // TODO: Allow people to change the url and username
    if let Some(key) = args.discord_key {
        let mut rng = thread_rng();
        let time = SystemTime::now()
            .add(Duration::from_secs(60 * 5))
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        // TODO: Unsure if time needs to be :?
        let content = format!("<bzzt> The server is due to restart at <t:{time:?}>, the following mods have been updated: <kssht>");

        let header = &args.user_agent.unwrap_or_default();

        for mut update in update_list.iter_mut() {
            for _ in 0..3 {
                let result = fetch_changelog(&update.publishedfileid, header).await;

                if let Ok(changelog) = result {
                    update.changelog = changelog;
                    break;
                } else if let Err(retry) = result {
                    if !retry {
                        break;
                    }

                    // If we need to retry, sleep on it
                    sleep(Duration::from_secs(3)).await
                }
            }

            if update.changelog.is_empty() {
                update.changelog =
                    "Change log was either not provided or could not be fetched.".to_string();
            }

            sleep(Duration::from_secs(3)).await;
        }

        let embeds = update_list
            .into_iter()
            .map(|update_list| DiscordEmbeds {
                title: update_list.title,
                color: {
                    let n: u16 = rng.gen();
                    n.to_string()
                },
                thumbnail: DiscordEmbedThumbnail {
                    url: update_list.preview_url,
                },
                url: format!(
                    "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
                    update_list.publishedfileid
                ),
                description: update_list.changelog,
            })
            .collect();

        let msg: DiscordWebhook = DiscordWebhook {
            avatar_url: "https://i.imgur.com/mXpcHBX.png",
            username: "Server maid",
            content: content.as_str(),
            embeds,
        };

        let client = reqwest::Client::new();
        client
            .post(format!("https://discord.com/api/webhooks/{key}"))
            .json(&msg)
            .send()
            .await?;
    }

    Ok(())
}

// Returns string or bool, bool means if it should retry
// TODO: Make error an actual error lol
async fn fetch_changelog(id: &String, header: &String) -> Result<String, bool> {
    let client = reqwest::ClientBuilder::new();
    let client = client.gzip(true).build();

    if let Err(why) = client {
        println!("Could not build reqwest client for changelog {id}, {why}");
        return Err(false);
    }

    let req = client
        .unwrap()
        .get(format!(
            "https://steamcommunity.com/sharedfiles/filedetails/changelog/{id}"
        ))
        .header(USER_AGENT, header)
        .send()
        .await;

    if let Err(why) = req {
        println!("Could not do changelog request for {id}, {why}");
        return Err(false);
    }

    let req = req.unwrap();

    if req.status() != 200 {
        println!(
            "Status for {id} was {}, reason: {:?}",
            req.status(),
            req.error_for_status()
        );

        return Err(true);
    }

    let text = req.text().await.expect("Could not convert text to utf8");

    let text = text.as_str();

    // Should probably print here? But if text is that big, that's just silly
    if text.len() > u32::MAX as usize {
        println!(
            "Text body bigger then {}, body was {}. Can not be parsed by tl.",
            u32::MAX,
            text
        );
        return Err(true);
    }

    let dom = tl::parse(text, tl::ParserOptions::default()).unwrap();

    let parser = dom.parser();
    let latest_cl = dom
        .query_selector(".workshopAnnouncement")
        .and_then(|mut iter| iter.next());

    // Retry, since we most likely got an error page
    if latest_cl.is_none() {
        println!("Could not find .workshopAnnouncement for id {id}, response was {text}");
        return Err(true);
    }

    let content = latest_cl.unwrap().get(parser).unwrap();
    let p = content
        .as_tag()
        .unwrap()
        .query_selector(parser, "p")
        .and_then(|mut iter| iter.next());

    // This really should not happen, since .workshopAnnouncement was found..
    if p.is_none() {
        println!("Could not find p tag for id {id}, response was {text}");
        return Err(false);
    }

    let text = p.unwrap().get(parser).unwrap().inner_html(parser);

    if text.is_empty() {
        println!("Mod {id} has no changelog, p tag is empty");
        return Err(false);
    }

    Ok(text.replace("<br></br>", "\n"))
}

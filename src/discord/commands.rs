use std::{num::NonZeroUsize, str::FromStr};

use super::{Context, Error, is_admin};
use anyhow::{Context as _, bail};
use poise::serenity_prelude as serenity;
use rand::RngExt;

/// Displays your or another user's account creation date
#[poise::command(slash_command, prefix_command)]
pub async fn age(
    ctx: Context<'_>,
    #[description = "Selected user"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let u = user.as_ref().unwrap_or_else(|| ctx.author());
    let response = format!("{}'s account was created at {}", u.name, u.created_at());
    ctx.say(response).await.context("Failed to respond")?;
    Ok(())
}

/// Replies to your message with a pong :)
#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("pong!").await.context("Failed to respond")?;
    Ok(())
}

/// Shuts down the server
#[poise::command(slash_command, prefix_command, check = "is_admin")]
pub async fn shutdown(ctx: Context<'_>) -> Result<(), Error> {
    // if this errors, skip aborting this function until shutdown signal is sent
    let res = ctx
        .reply("o7")
        .await
        .map(|_| ())
        .context("failed to send shutdown advisement message");
    ctx.data().shutdown().await?;
    res
}

/// Rolls die based off the die roll notation, defaulting to `1d20`
#[poise::command(slash_command, prefix_command)]
pub async fn roll(
    ctx: Context<'_>,
    // Ex. `1d20 - 6`
    notation: Option<String>,
) -> Result<(), Error> {
    enum NotationEntry {
        Plus,
        Minus,
        Number(isize),
        Roll {
            count: NonZeroUsize,
            sides: NonZeroUsize,
        },
    }
    impl FromStr for NotationEntry {
        type Err = anyhow::Error;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            if s.contains(' ') {
                bail!("Spaces should be filtered out by the time the string is parsed")
            }
            if s.contains('d') {
                let Some((count, sides)) = s.split_once('d') else {
                    bail!(
                        "expected [number]d[number], got a string that starts with or ends with d"
                    )
                };
                let count = count.parse::<NonZeroUsize>().context("Invalid count")?;
                let sides = sides.parse::<NonZeroUsize>().context("Invalid sides")?;
                return Ok(Self::Roll { count, sides });
            } else if s == "+" {
                return Ok(Self::Plus);
            } else if s == "-" {
                return Ok(Self::Minus);
            }
            let num = s.parse::<isize>().context("Invalid number")?;
            Ok(Self::Number(num))
        }
    }
    let notation_entries = notation
        .unwrap_or("1d20".to_string())
        .split(' ')
        .map(NotationEntry::from_str)
        .collect::<anyhow::Result<Vec<NotationEntry>>>()
        .context("Failed to parse one or more parts of the notation")?;

    let mut total: isize = 0;
    let mut output_str = String::new();
    let mut is_addition = true;

    {
        let mut rng = rand::rng();
        for entry in notation_entries {
            match entry {
                NotationEntry::Plus => {
                    is_addition = true;
                    output_str.push_str(" + ");
                }
                NotationEntry::Minus => {
                    is_addition = false;
                    output_str.push_str(" - ");
                }
                NotationEntry::Number(num) => {
                    if is_addition {
                        total += num;
                    } else {
                        total -= num;
                    }
                    output_str.push_str(&num.to_string());
                }
                NotationEntry::Roll { count, sides } => {
                    let mut rolls = Vec::new();
                    let mut roll_sum = 0;

                    for _ in 0..count.get() {
                        let roll = rng.random_range(1_i64..=sides.get() as i64) as isize;
                        rolls.push(roll.to_string());
                        roll_sum += roll;
                    }

                    if is_addition {
                        total += roll_sum;
                    } else {
                        total -= roll_sum;
                    }

                    // Format as: 2d20 (1, 15)
                    let rolls_joined = rolls.join(", ");
                    output_str.push_str(&format!("{}d{} ({})", count, sides, rolls_joined));
                }
            }
        }
    }

    ctx.say(format!("🎲 {} = **{}**", output_str, total))
        .await?;

    Ok(())
}

/// Flips a coin, returns heads or tails
#[poise::command(slash_command, prefix_command)]
pub async fn coin_flip(ctx: Context<'_>) -> Result<(), Error> {
    let result = {
        let mut rng = rand::rng();
        match rng.random_bool(0.5) {
            true => "heads",
            false => "tails",
        }
    };
    ctx.reply(format!("The coin landed on {result}"))
        .await
        .context("Failed to respond")?;
    Ok(())
}

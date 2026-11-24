use std::{env, sync::Arc, time::Duration};

use chrono::{DateTime, Days, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use log::{error, info};
use serenity::{all::GenericChannelId, http::Http};

use crate::{
    bot::Handler,
    command::{leaderboard, open_markets},
    utils,
};

const POST_TIME: NaiveTime = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
const DEFAULT_TIMEZONE: &str = "America/Toronto";

pub struct DailyDigestConfig {
    pub channel: GenericChannelId,
    pub timezone: Tz,
}

impl DailyDigestConfig {
    pub fn from_env() -> Option<Self> {
        let channel = env::var("DAILY_DIGEST_CHANNEL_ID")
            .ok()
            .filter(|v| !v.is_empty())?
            .parse::<u64>()
            .expect("DAILY_DIGEST_CHANNEL_ID should be a valid u64");

        let timezone = env::var("DAILY_DIGEST_TIMEZONE")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_TIMEZONE.to_string())
            .parse::<Tz>()
            .expect("DAILY_DIGEST_TIMEZONE should be a valid IANA time zone");

        Some(Self {
            channel: GenericChannelId::new(channel),
            timezone,
        })
    }
}

fn next_post_time(now: DateTime<Utc>, tz: Tz) -> DateTime<Utc> {
    let local_now = now.with_timezone(&tz);
    let mut date = local_now.date_naive();

    loop {
        if let Some(candidate) = tz.from_local_datetime(&date.and_time(POST_TIME)).earliest()
            && candidate > local_now
        {
            return candidate.with_timezone(&Utc);
        }
        date = date + Days::new(1);
    }
}

pub async fn post(handler: &Handler, http: &Http, channel: GenericChannelId) -> anyhow::Result<()> {
    let markets = open_markets::render(handler).await?;
    let leaderboard = leaderboard::render(handler, leaderboard::LeaderboardField::NetWorth).await?;

    utils::send_long_channel_message(http, channel, &markets).await?;
    utils::send_long_channel_message(
        http,
        channel,
        &format!("## NET WORTH LEADERBOARD\n{leaderboard}"),
    )
    .await
}

pub async fn run(handler: Arc<Handler>, http: Arc<Http>, config: DailyDigestConfig) {
    loop {
        let now = Utc::now();
        let next = next_post_time(now, config.timezone);
        info!("Next daily digest post at {next}");

        let wait = (next - now).to_std().unwrap_or(Duration::ZERO);
        tokio::time::sleep(wait).await;

        match post(&handler, &http, config.channel).await {
            Ok(()) => info!("Posted daily digest to channel {}", config.channel),
            Err(err) => error!("Error posting daily digest: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn later_same_day() {
        assert_eq!(
            next_post_time(utc("2026-11-24T12:00:00Z"), chrono_tz::America::Toronto),
            utc("2026-11-24T14:00:00Z")
        );
    }

    #[test]
    fn rolls_to_next_day_after_post_time() {
        assert_eq!(
            next_post_time(utc("2026-11-24T14:00:00Z"), chrono_tz::America::Toronto),
            utc("2026-11-25T14:00:00Z")
        );
    }

    #[test]
    fn follows_daylight_saving() {
        assert_eq!(
            next_post_time(utc("2026-07-01T00:00:00Z"), chrono_tz::America::Toronto),
            utc("2026-07-01T13:00:00Z")
        );
    }

    #[test]
    fn uses_local_date_not_utc_date() {
        assert_eq!(
            next_post_time(utc("2026-11-25T02:00:00Z"), chrono_tz::America::Toronto),
            utc("2026-11-25T14:00:00Z")
        );
    }
}

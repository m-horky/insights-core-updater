use insights_core_updater::{Core, CoreInfo};
use log;
use simplelog::LevelPadding;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;

const LOG_FILE_PATH: &str = "insights-core-updater.log";
const CORE_FILE_PATH: &str = "insights-core.egg";
const CORE_SIGNATURE_FILE_PATH: &str = "insights-core.egg.asc";
const CACHE_FILE_PATH: &str = "insights-core-updater.cache";


fn set_up_logging() {
    // TODO .set_time_offset()
    //  set_time_offset_to_local() doesn't work, it doesn't know where we are
    let mut fp: File = OpenOptions::new().create(true).append(true).open(LOG_FILE_PATH).unwrap();
    _ = fp.write("\n".as_bytes());

    let config = simplelog::ConfigBuilder::new()
        .set_max_level(simplelog::LevelFilter::Error)
        .set_level_padding(LevelPadding::Right)
        .set_thread_level(simplelog::LevelFilter::Off)
        .set_location_level(simplelog::LevelFilter::Off)
        .set_target_level(simplelog::LevelFilter::Error)
        .set_level_color(simplelog::Level::Error, Some(simplelog::Color::Red))
        .set_level_color(simplelog::Level::Warn, Some(simplelog::Color::Yellow))
        .set_level_color(simplelog::Level::Info, Some(simplelog::Color::Green))
        .set_level_color(simplelog::Level::Debug, Some(simplelog::Color::Blue))
        .set_level_color(simplelog::Level::Trace, Some(simplelog::Color::Cyan))
        .add_filter_allow_str("insights_core_updater")
        .set_time_format_custom(simplelog::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]+[offset_hour]:[offset_minute]"))
        .build();

    let term = simplelog::TermLogger::new(simplelog::LevelFilter::Debug, config.clone(), simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto);
    let file = simplelog::WriteLogger::new(simplelog::LevelFilter::Debug, config.clone(), fp);

    if env::var("DEBUG").is_ok() {
        _ = simplelog::CombinedLogger::init(vec![term, file]);
    } else {
        _ = simplelog::CombinedLogger::init(vec![file]);
    }
}

#[tokio::main]
async fn main() {
    set_up_logging();

    if !insights_core_updater::is_registered() {
        return;
    }

    let cache: Option<CoreInfo> = CoreInfo::from_cache(CACHE_FILE_PATH);
    let cache: CoreInfo = cache.unwrap_or_else(|| CoreInfo::new());

    let core: Option<Core> = Core::fetch().await;
    if let None = core {
        println!("Core could not be fetched.");
        return;
    }
    let mut core = core.unwrap();

    if cache.etag == core.info.etag {
        log::info!("ETag values match, no need to do anything.");
        println!("Nothing to do.");
        return;
    }
    log::debug!(
        "The egg changed on \"{}\", its etag is {}.",
        core.info.last_modified.clone().unwrap_or("?".to_string()),
        core.info.etag.clone().unwrap_or("?".to_string()),
    );
    if core.fetch_signature().await.is_none() {
        log::error!("Could not fetch signature.");
        return;
    }

    if core.cache(CORE_FILE_PATH, CORE_SIGNATURE_FILE_PATH).is_none() {
        log::error!("Could not save core and its signature.");
        return;
    }
    if core.info.cache(CACHE_FILE_PATH).is_none() {
        log::error!("Could not save Core cache.");
        return;
    }

    println!("New Core saved at {}.", CORE_FILE_PATH);
}

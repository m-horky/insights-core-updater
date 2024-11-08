use insights_core_updater::{Core, CoreInfo};
use log;
use simplelog::LevelPadding;
use std::fs::File;

const LOG_FILE_PATH: &str = "insights-core-updater.log";
const CORE_FILE_PATH: &str = "insights-core.egg";
const CACHE_FILE_PATH: &str = "insights-core-updater.json";


fn set_up_logging() {
    // TODO Log all logs, not just the updater, if some envvar is set
    // TODO .set_time_offset()
    //  set_time_offset_to_local() doesn't work, it doesn't know where we are
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

    _ = simplelog::CombinedLogger::init(
        vec![
            simplelog::TermLogger::new(simplelog::LevelFilter::Debug, config.clone(), simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto),
            simplelog::WriteLogger::new(simplelog::LevelFilter::Debug, config.clone(), File::create(LOG_FILE_PATH).unwrap()),
        ]
    );
}

#[tokio::main]
async fn main() {
    set_up_logging();

    if !insights_core_updater::is_registered() {
        return;
    }

    let fresh_core_info: Option<CoreInfo> = CoreInfo::fetch().await;
    if let None = fresh_core_info {
        println!("Information about Core could not be fetched.");
        return;
    }
    let fresh_core_info = fresh_core_info.unwrap();

    let cached_core_info: Option<CoreInfo> = CoreInfo::from_cache(CACHE_FILE_PATH);
    let cached_core_info: CoreInfo = cached_core_info.unwrap_or_else(|| CoreInfo::new());

    if fresh_core_info.etag == cached_core_info.etag {
        log::info!("ETag values match, no need to do anything.");
        return;
    }

    let core: Option<Core> = Core::fetch().await;
    if let None = core {
        println!("Core could not be fetched.");
        return;
    }
    let core = core.unwrap();

    core.cache(CORE_FILE_PATH);
    fresh_core_info.cache(CACHE_FILE_PATH);
    println!("New Core saved at {}.", CORE_FILE_PATH);
}

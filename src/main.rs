mod apps;
mod config;
mod entry;
mod kde;
mod launch;
mod providers;
mod search;
mod single;
mod style;
mod theme;
mod ui;
mod usage;

pub const APP_ID: &str = "io.github.fredrir.quicklaunch";

fn main() -> Result<(), iced_layershell::Error> {
    let args: Vec<String> = std::env::args().collect();
    let config = config::Config::load();

    if let Some(pos) = args.iter().position(|a| a == "--list") {
        debug_list(&config, args.get(pos + 1).map(String::as_str));
        return Ok(());
    }

    if args.iter().any(|a| a == "--theme") {
        debug_theme(&config);
        return Ok(());
    }

    let initial_query = args
        .iter()
        .position(|a| a == "--query")
        .and_then(|pos| args.get(pos + 1))
        .cloned();

    if single::toggle_or_register() {
        return Ok(());
    }

    let result = ui::run(config, initial_query);
    single::cleanup();
    result
}

fn debug_theme(config: &config::Config) {
    let t = theme::Theme::resolve(&config.theme);
    let hex = |c: iced::Color| {
        format!(
            "#{:02x}{:02x}{:02x}",
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
        )
    };
    println!("source      {:?}", config.theme.source);
    println!("background  {}", hex(t.bg));
    println!("text        {}", hex(t.text));
    println!("muted       {}", hex(t.muted));
    println!("accent      {}", hex(t.accent));
    println!("selection   {}", hex(t.selection));
    println!("placeholder {}", hex(t.placeholder));
}

fn debug_list(config: &config::Config, query: Option<&str>) {
    let apps = providers::load(&config.plugins);
    let usage = usage::Usage::load();
    println!("indexed {} entries", apps.len());

    match query {
        Some(q) => {
            println!("results for {q:?}:");
            let ranked = search::rank(
                q,
                &apps,
                config.window.max_results,
                &usage,
                config.behavior.frequency_ranking,
            );
            for i in ranked {
                let app = &apps[i];
                println!(
                    "  {:<28} icon={:<24} term={}",
                    app.name,
                    app.icon.as_deref().unwrap_or("-"),
                    app.terminal(),
                );
            }
        }
        None => {
            for app in apps.iter().take(15) {
                println!("  {}", app.name);
            }
            println!("  … ({} total)", apps.len());
        }
    }
}

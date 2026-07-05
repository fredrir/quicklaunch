//! kde-app-launcher — a minimal, Spotlight-style application launcher for KDE
//! Plasma on Wayland, built on iced + wlr-layer-shell.

mod apps;
mod launch;
mod search;
mod single;
mod style;
mod ui;

fn main() -> Result<(), iced_layershell::Error> {
    let args: Vec<String> = std::env::args().collect();

    // Headless debug path: `kde-app-launcher --list [query]` prints the index and,
    // if a query is given, the ranked matches. Useful for verifying discovery and
    // ranking without bringing up the GUI.
    if let Some(pos) = args.iter().position(|a| a == "--list") {
        debug_list(args.get(pos + 1).map(String::as_str));
        return Ok(());
    }

    // Optional `--query <text>` pre-fills the search field on boot.
    let initial_query = args
        .iter()
        .position(|a| a == "--query")
        .and_then(|pos| args.get(pos + 1))
        .cloned();

    // Toggle semantics: if an instance is already showing, dismiss it and exit.
    if single::toggle_or_register() {
        return Ok(());
    }

    let result = ui::run(initial_query);
    single::cleanup();
    result
}

fn debug_list(query: Option<&str>) {
    let apps = apps::index_apps();
    println!("indexed {} applications", apps.len());

    match query {
        Some(q) => {
            println!("results for {q:?}:");
            for i in search::rank(q, &apps, style::MAX_RESULTS) {
                let app = &apps[i];
                println!(
                    "  {:<28} icon={:<24} term={}",
                    app.name,
                    app.icon.as_deref().unwrap_or("-"),
                    app.terminal,
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

mod config;
mod discovery;
mod policy;
mod recent;
mod ui;
mod workspaces;

fn main() -> std::io::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("herdr-workspaces-manager {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h") => {
            println!(
                "Herdr workspaces manager: run without arguments to open the project picker.\nConfiguration: $(herdr plugin config-dir herdr-workspaces-manager)/config.json"
            );
            Ok(())
        }
        Some(argument) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unknown argument: {argument}"),
        )),
        None => ui::run(),
    }
}

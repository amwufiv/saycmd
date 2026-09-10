use std::{
    env,
    path::{Path, PathBuf},
    process::{self, Command},
};

use clap::{Parser, Subcommand};
use saycmd::{
    api::{ChatClient, PreviousCommand},
    config::{config_path, load_prefix, Config},
    error::{Result, SaycmdError},
    shell::zsh_init,
};

#[derive(Parser)]
#[command(name = "saycmd", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Translate natural language into a reviewable shell command or script.
    Translate {
        /// Revise this command instead of starting a new request.
        #[arg(long)]
        previous_command: Option<String>,
        /// Previous request in this conversation (repeatable, up to eight).
        #[arg(long, requires = "previous_command")]
        previous_request: Vec<String>,
        /// Override whether to explain command arguments.
        #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
        explain: Option<bool>,
        #[arg(long, hide = true)]
        zle: bool,
        #[arg(long, default_value = "zsh")]
        shell: String,
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        prompt: Vec<String>,
    },
    /// Print shell integration code for eval/source.
    Init {
        #[arg(value_parser = ["zsh"])]
        shell: String,
    },
    /// Check local configuration, Zsh availability, and API connectivity.
    Doctor,
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("saycmd: {error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Translate {
            shell,
            cwd,
            prompt,
            explain,
            zle,
            previous_command,
            previous_request,
        } => {
            let cwd = valid_cwd(&cwd)?;
            let mut config = Config::load()?;
            if let Some(explain) = explain {
                config.explain = explain;
            }
            let previous = previous_command.map(|command| PreviousCommand {
                command,
                requests: previous_request,
            });
            let result = saycmd::translate_with_previous(
                &config,
                &shell,
                &cwd,
                &prompt.join(" "),
                previous.as_ref(),
            )?;
            if zle {
                print!(
                    "{}\0{}",
                    result.command,
                    result.explanation.unwrap_or_default()
                );
            } else {
                print!("{}", result.command);
                if let Some(explanation) = result.explanation {
                    eprintln!("{explanation}");
                }
            }
            Ok(())
        }
        Commands::Init { shell } => {
            debug_assert_eq!(shell, "zsh");
            print!("{}", zsh_init(&load_prefix()?));
            Ok(())
        }
        Commands::Doctor => doctor(),
    }
}

fn valid_cwd(path: &Path) -> Result<PathBuf> {
    let path = if path == Path::new(".") {
        env::current_dir().map_err(|error| SaycmdError::InvalidCwd(error.to_string()))?
    } else {
        path.to_owned()
    };
    if !path.is_dir() {
        return Err(SaycmdError::InvalidCwd(path.display().to_string()));
    }
    path.canonicalize()
        .map_err(|error| SaycmdError::InvalidCwd(error.to_string()))
}

fn doctor() -> Result<()> {
    let config = Config::load()?;
    println!("ok config: {}", config_path().display());
    println!("ok model: {}", config.model);
    println!("ok endpoint: {}", config.base_url);

    let zsh = Command::new("zsh")
        .arg("--version")
        .output()
        .map_err(|source| SaycmdError::Process {
            program: "zsh".to_owned(),
            source,
        })?;
    if !zsh.status.success() {
        return Err(SaycmdError::InvalidSyntax {
            shell: "zsh".to_owned(),
            message: "zsh --version failed".to_owned(),
        });
    }
    println!("ok shell: {}", String::from_utf8_lossy(&zsh.stdout).trim());

    ChatClient::new(&config)?.check_connection()?;
    println!("ok API connectivity");
    Ok(())
}

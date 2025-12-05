use std::num::NonZeroU64;

use aws_config::BehaviorVersion;
use aws_sdk_secretsmanager::{
    operation::{create_secret::CreateSecretError, delete_secret::DeleteSecretError},
};
use clap::{Parser, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

#[derive(Parser)]
#[command(name = "secret-populator")]
#[command(about = "Manage secrets in AWS Secrets Manager")]
struct Args {
    #[arg(long)]
    endpoint_url: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Create {
        #[arg(short, long, default_value = "10")]
        count: NonZeroU64,
        #[arg(short, long, default_value = "generated-secret")]
        prefix: String,
    },
    Delete {
        #[arg(short, long, default_value = "10")]
        count: NonZeroU64,
        #[arg(short, long, default_value = "generated-secret")]
        prefix: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let mut client_config = aws_sdk_secretsmanager::config::Builder::from(&config);
    client_config.set_endpoint_url(args.endpoint_url);
    let client = aws_sdk_secretsmanager::Client::from_conf(client_config.build());

    match args.command {
        Command::Create { count, prefix } => {
            let mp = MultiProgress::new();
            let pb = mp.add(
                ProgressBar::new(count.get()).with_style(
                    ProgressStyle::with_template(
                        "[{elapsed_precise}] {bar:60.cyan/blue} {pos:>7}/{len:7} {msg}",
                    )
                    .unwrap(),
                ),
            );

            pb.set_draw_target(ProgressDrawTarget::stderr_with_hz(10));

            let mut errors: u64 = 0;
            for i in pb.wrap_iter(1..=count.get()) {
                let name = format!("{}-{}", prefix, i);

                match client
                    .create_secret()
                    .name(&name)
                    .secret_string(format!("secret-value-{}", i))
                    .send()
                    .await
                {
                    Ok(_) => pb.set_message(format!("Created secret: {}", name)),
                    Err(e) => match e.into_service_error() {
                        CreateSecretError::ResourceExistsException(_) => {
                            pb.println(format!("Secret already exists: {}", name));
                            errors = errors + 1;
                        }
                        err => return Err(err.into()),
                    },
                }
            }

            if errors == 0 {
                pb.set_message(format!("Created {count} secrets"));
            }

            pb.finish();
        }
        Command::Delete { count, prefix } => {
            let mp = MultiProgress::new();
            let pb = mp.add(
                ProgressBar::new(count.get()).with_style(
                    ProgressStyle::with_template(
                        "[{elapsed_precise}] {bar:60.cyan/blue} {pos:>7}/{len:7} {msg}",
                    )
                    .unwrap(),
                ),
            );

            for i in pb.wrap_iter(1..=count.get()) {
                let name = format!("{}-{}", prefix, i);

                match client
                    .delete_secret()
                    .secret_id(&name)
                    .force_delete_without_recovery(true)
                    .send()
                    .await
                {
                    Ok(_) => pb.set_message(format!("Deleted secret: {}", name)),
                    Err(e) => match e.into_service_error() {
                        DeleteSecretError::ResourceNotFoundException(_) => {
                            pb.println(format!("Secret not found: {}", name));
                        }
                        err => return Err(err.into()),
                    },
                }
            }

            pb.set_message(format!("Deleted {count} secrets"));
            pb.finish();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    #[test]
    fn test_count_zero_errors() {
        use clap::CommandFactory;
        super::Args::command().debug_assert();
        let result = super::Args::try_parse_from(&["secret-populator", "create", "--count", "0"]);
        assert!(result.is_err());
    }
}

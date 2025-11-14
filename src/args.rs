use clap::Parser;

#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    // Web server address
    #[arg(long, env, default_value = "0.0.0.0:8888")]
    pub(crate) host: String,

    // Path to the users file
    #[arg(long, env, default_value = "./tmp/users.json")]
    pub(crate) users_file: String,
}

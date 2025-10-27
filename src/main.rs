use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose};
use clap::{Args, Parser, ValueEnum, builder::Styles};
use colored::*;
use reqwest::{
    ClientBuilder,
    header::{AUTHORIZATION, CONTENT_TYPE},
};

#[derive(Debug, Copy, Clone, ValueEnum)]
enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    PATCH,
    TRACE,
    CONNECT,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::GET => write!(f, "get"),
            Method::POST => write!(f, "post"),
            Method::PUT => write!(f, "put"),
            Method::DELETE => write!(f, "delete"),
            Method::PATCH => write!(f, "patch"),
            Method::HEAD => write!(f, "head"),
            Method::OPTIONS => write!(f, "options"),
            Method::TRACE => write!(f, "trace"),
            Method::CONNECT => write!(f, "connect"),
        }
    }
}

impl From<Method> for reqwest::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::GET => reqwest::Method::GET,
            Method::POST => reqwest::Method::POST,
            Method::PUT => reqwest::Method::PUT,
            Method::DELETE => reqwest::Method::DELETE,
            Method::PATCH => reqwest::Method::PATCH,
            Method::HEAD => reqwest::Method::HEAD,
            Method::OPTIONS => reqwest::Method::OPTIONS,
            Method::TRACE => reqwest::Method::TRACE,
            Method::CONNECT => reqwest::Method::CONNECT,
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about)]
#[command(styles = styles())]
struct Cli {
    /// The URL to request.
    url: String,

    #[arg(short, long, default_value_t = Method::GET)]
    method: Method,

    /// Add a header to the request.
    #[arg(short = 'H', long = "header", value_name = "NAME=VALUE")]
    headers: Vec<String>,

    /// Add a cookie to the request.
    #[arg(short = 'c', long = "cookie", value_name = "NAME=VALUE")]
    cookies: Vec<String>,

    #[command(flatten)]
    auth_type: AuthType,

    /// Set the request body.
    /// Multiple occurrences are concatenated.
    #[arg(short, long)]
    bodies: Vec<String>,

    #[command(flatten)]
    body_type: BodyType,
}

#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
struct AuthType {
    /// Short hand notation for the `Authorization` header.
    #[arg(short = 'a', long = "auth")]
    auth: Option<String>,

    /// HTTP Basic Authentication in the format username:password.
    /// If password is omitted, you will be prompted for it.
    #[arg(long = "user", value_name = "USER[:PASSWORD]")]
    user: Option<String>,
}

#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
struct BodyType {
    /// The body is JSON.
    /// Sets the `content-type=application/json` header.
    /// Denies the request if the body is syntactically malformed.
    /// Multiple bodies are concatenated.
    #[arg(short = 'j', long = "json-body")]
    json: bool,

    /// The body is URL encoded.
    /// Sets the `content-type=application/x-www-form-urlencoded` header.
    /// Multiple bodies are concatenated with a `&` between them.
    #[arg(short = 'u', long = "url-body")]
    url_encoded: bool,

    /// The body is a multipart form.
    /// Sets the `content-type=multipart/form-data` header.
    /// Multiple occurrences are allowed.
    #[arg(short = 'f', long = "form-body")]
    form: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{} {error}", "Error:".red());
        std::process::exit(1);
    }
}

#[derive(Debug)]
enum Error {
    InvalidJson(serde_json5::Error),
    InvalidHeader(String),
    InvalidFormField(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidHeader(header) => write!(f, "Invalid header: \"{header}\""),
            Error::InvalidJson(json_err) => write!(f, "Invalid JSON: {json_err}"),
            Error::InvalidFormField(field) => write!(f, "Invalid form field: \"{field}\""),
        }
    }
}

impl std::error::Error for Error {}

async fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let url = reqwest::Url::parse(&args.url)?;

    let jar = reqwest::cookie::Jar::default();
    for cookie in &args.cookies {
        jar.add_cookie_str(&cookie, &url);
    }

    let client = ClientBuilder::new()
        .cookie_provider(Arc::new(jar))
        .build()?;

    let method = args.method.into();

    let mut request = client.request(method, url);

    for header in &args.headers {
        let (name, value) = header
            .split_once('=')
            .ok_or_else(|| Error::InvalidHeader(header.clone()))?;
        request = request.header(name, value);
    }

    if let Some(auth) = args.auth_type.auth {
        request = request.header(AUTHORIZATION, auth);
    }

    if let Some(user) = args.auth_type.user {
        let auth_value = if user.contains(':') {
            let encoded = general_purpose::STANDARD.encode(user.as_bytes());
            format!("Basic {}", encoded)
        } else {
            let password = rpassword::prompt_password(format!("Enter password for {}: ", user))?;
            let credentials = format!("{}:{}", user, password);
            let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
            format!("Basic {}", encoded)
        };
        request = request.header(AUTHORIZATION, auth_value);
    }

    if args.body_type.url_encoded {
        request = request.header(CONTENT_TYPE, "application/x-www-form-urlencoded");
    }
    if args.body_type.json {
        request = request.header(CONTENT_TYPE, "application/json");
    }

    if args.body_type.form {
        let mut form = reqwest::multipart::Form::new();
        for field in &args.bodies {
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| Error::InvalidFormField(field.clone()))?;
            form = form.text(key.to_string(), value.to_string());
        }
        request = request.multipart(form);
    } else {
        let mut concatenated_body = String::new();
        for body in args.bodies {
            if args.body_type.json {
                if let Err(err) = serde_json5::from_str::<serde_json::Value>(&body) {
                    return Err(Box::new(Error::InvalidJson(err)));
                }
            }
            if args.body_type.url_encoded {
                concatenated_body.push('&');
            }
            concatenated_body += &body;
        }
        request = request.body(concatenated_body);
    }

    let response = client.execute(request.build()?).await?;

    let status = response.status();

    println!(
        "{} {}",
        "Status:".bold().underline(),
        status
            .to_string()
            .bold()
            .color(if status.is_informational() {
                Color::Blue
            } else if status.is_success() {
                Color::Green
            } else if status.is_redirection() {
                Color::Yellow
            } else if status.is_client_error() || status.is_server_error() {
                Color::Red
            } else {
                Color::White
            })
    );

    let mut is_json = false;

    println!("{}", "Headers:".bold().underline());
    for (key, value) in response.headers().iter() {
        println!("  {}: {:?}", key.to_string().bold(), value);

        if key == CONTENT_TYPE {
            if value
                .to_str()
                .is_ok_and(|value| value.contains("application/json"))
            {
                is_json = true;
            }
        }
    }

    println!("{}", "Body:".bold().underline());
    let body = response.text().await?;
    if is_json {
        let body: serde_json::Value = serde_json5::from_str(&body)?;
        pretty_print(&body, 0);
        println!();
    } else {
        println!("{body}");
    }

    Ok(())
}

fn print_indent(depth: usize) {
    for _ in 0..depth {
        print!("  ");
    }
}

fn pretty_print(value: &serde_json::Value, depth: usize) {
    match value {
        serde_json::Value::Null => print!("{}", "null".bright_magenta()),
        serde_json::Value::Bool(bool) => print!("{}", bool.to_string().bright_purple()),
        serde_json::Value::Number(number) => print!("{}", number.to_string().bright_cyan()),
        serde_json::Value::String(string) => {
            print!(
                "{}{}{}",
                "\"".bright_green(),
                string.bright_green(),
                "\"".bright_green()
            )
        }
        serde_json::Value::Array(values) => {
            print!("{}", "[".bright_black());
            let multi_line = !values.is_empty();
            if multi_line {
                println!();
            }
            for value in values.iter() {
                print_indent(depth + 1);
                pretty_print(value, depth + 1);
                println!("{}", ",".bright_black());
            }
            if multi_line {
                print_indent(depth);
            }
            print!("{}", "]".bright_black());
        }
        serde_json::Value::Object(map) => {
            println!("{}", "{".bright_black());
            for (key, value) in map.iter() {
                print_indent(depth + 1);
                print!(
                    "{}{}{} ",
                    "\"".bright_black(),
                    key.bold().bright_yellow(),
                    "\":".bright_black(),
                );
                pretty_print(value, depth + 1);
                println!("{}", ",".bright_black());
            }
            print_indent(depth);
            print!("{}", "}".bright_black());
        }
    }
}

fn styles() -> Styles {
    Styles::styled()
        .header(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
        )
        .usage(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
        )
        .literal(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
        )
        .placeholder(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Cyan))),
        )
        .error(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .valid(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
        )
        .invalid(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
}

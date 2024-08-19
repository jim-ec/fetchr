use std::sync::Arc;

use clap::{Parser, ValueEnum};
use colored::*;
use reqwest::{
    header::{HeaderName, HeaderValue},
    ClientBuilder, Request,
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
struct Cli {
    /// The URL to request
    url: String,

    #[arg(short, long, default_value_t = Method::GET)]
    method: Method,

    /// Add a header to the request
    #[arg(short = 'H', long = "header", value_name = "NAME=VALUE")]
    headers: Vec<String>,

    /// Add a cookie to the request
    #[arg(short = 'c', long = "cookie", value_name = "NAME=VALUE")]
    cookies: Vec<String>,

    /// Set the request body
    #[arg(short, long)]
    body: Option<String>,
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => {}
        Err(error) => {
            eprintln!("{} {}", "Error:".red(), error.to_string().red());
            std::process::exit(1);
        }
    }
}

#[derive(Debug)]
enum ParserError {
    InvalidHeader(String),
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::InvalidHeader(header) => write!(f, "invalid header: \"{header}\""),
        }
    }
}

impl std::error::Error for ParserError {}

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

    let mut request = Request::new(method, url);

    for header in &args.headers {
        let mut parts = header.split('=');
        let name = parts
            .next()
            .ok_or_else(|| ParserError::InvalidHeader(header.clone()))?;
        let value = parts
            .next()
            .ok_or_else(|| ParserError::InvalidHeader(header.clone()))?;
        let name: HeaderName = name.parse()?;
        dbg!(&name);
        let value: HeaderValue = value.parse()?;
        dbg!(&value);
        request.headers_mut().insert(name, value);
    }

    if let Some(body) = args.body {
        *request.body_mut() = Some(body.into());
    }

    let response = client.execute(request).await?;

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

        if key.as_str().contains("content-type") {
            if value
                .to_str()
                .map(|value| value == "application/json")
                .unwrap_or(false)
            {
                is_json = true;
            }
        }
    }

    if is_json {
        println!("{}", "Body (JSON):".bold().underline());
        let body: serde_json::Value = response.json().await?;
        pretty_print(&body, 0);
        println!();
    } else {
        println!("{}", "Body:".bold().underline());
        let body = response.text().await?;
        println!("{}", body);
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
            for value in values.iter().rev().skip(1).rev() {
                print_indent(depth + 1);
                pretty_print(value, depth + 1);
                println!("{}", ",".bright_black());
            }
            if let Some(value) = values.last() {
                print_indent(depth + 1);
                pretty_print(value, depth + 1);
                println!();
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

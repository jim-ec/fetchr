use std::{
    io::{Read, Write},
    process::ExitCode,
    sync::Arc,
};

use base64::prelude::*;
use clap::Parser;
use colored::*;
use reqwest::{
    blocking::{ClientBuilder, multipart::Form},
    header::{AUTHORIZATION, CONTENT_TYPE},
    redirect::Policy,
};

mod cli;

#[derive(Debug, Clone)]
enum BodyContent {
    String(String),
    Binary(Vec<u8>),
}

impl BodyContent {
    fn to_string(self) -> Result<String, std::string::FromUtf8Error> {
        match self {
            BodyContent::String(s) => Ok(s),
            BodyContent::Binary(bytes) => String::from_utf8(bytes),
        }
    }

    fn to_bytes(self) -> Vec<u8> {
        match self {
            BodyContent::String(s) => s.into_bytes(),
            BodyContent::Binary(bytes) => bytes,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} {error}", "Error:".red());
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
enum Error {
    InvalidJson(serde_json5::Error),
    InvalidHeader(String),
    InvalidFormField(String),
    InvalidQueryParam(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidHeader(header) => write!(f, "Invalid header: \"{header}\""),
            Error::InvalidJson(json_err) => write!(f, "Invalid JSON: {json_err}"),
            Error::InvalidFormField(field) => write!(f, "Invalid form field: \"{field}\""),
            Error::InvalidQueryParam(param) => write!(f, "Invalid query parameter: \"{param}\""),
        }
    }
}

impl std::error::Error for Error {}

fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = cli::Cli::parse();

    let mut url = reqwest::Url::parse(&args.url)?;

    // Add query parameters
    for query in &args.query_params {
        let (key, value) = query
            .split_once('=')
            .ok_or_else(|| Error::InvalidQueryParam(query.clone()))?;
        url.query_pairs_mut().append_pair(key, value);
    }

    let jar = reqwest::cookie::Jar::default();
    for cookie in &args.cookies {
        jar.add_cookie_str(&cookie, &url);
    }

    let redirect_policy = if args.no_follow_redirects {
        Policy::none()
    } else {
        Policy::limited(args.max_redirects)
    };

    let client = ClientBuilder::new()
        .cookie_provider(Arc::new(jar))
        .redirect(redirect_policy)
        .build()?;

    let method = args.method.into();

    let mut request = client.request(method, url);

    for header in &args.headers {
        let (name, value) = header
            .split_once('=')
            .ok_or_else(|| Error::InvalidHeader(header.clone()))?;
        request = request.header(name, value);
    }

    if let Some(auth) = args.auth_method.auth {
        request = request.header(AUTHORIZATION, auth);
    }

    if let Some(user) = args.auth_method.user {
        let auth_value = if user.contains(':') {
            let encoded = BASE64_STANDARD.encode(user.as_bytes());
            format!("Basic {}", encoded)
        } else {
            let password = rpassword::prompt_password(format!("Enter password for {}: ", user))?;
            let credentials = format!("{}:{}", user, password);
            let encoded = BASE64_STANDARD.encode(credentials.as_bytes());
            format!("Basic {}", encoded)
        };
        request = request.header(AUTHORIZATION, auth_value);
    }

    // if args.url_encoded {
    //     request = request.header(CONTENT_TYPE, "application/x-www-form-urlencoded");
    // }
    if args.json {
        request = request.header(CONTENT_TYPE, "application/json");
    }

    request = if let Some(string) = args.body_source.string {
        request.body(string)
    } else if let Some(path) = args.body_source.path {
        if *path == *cli::STDIN {
            let mut buffer = Vec::new();
            std::io::stdin().read_to_end(&mut buffer)?;
            request.body(buffer)
        } else {
            let mut file = std::fs::File::open(path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            request.body(buffer)
        }
    } else if let Some(form_fields) = args.body_source.form_fields {
        let mut form = Form::new();
        for field in form_fields {
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| Error::InvalidFormField(field.clone()))?;
            form = form.text(key.to_string(), value.to_string());
        }
        request.multipart(form)
    } else {
        request
    };

    // if args.json {
    //     let body = body.clone().to_string()?;
    //     if let Err(err) = serde_json5::from_str::<serde_json::Value>(&body) {
    //         return Err(Box::new(Error::InvalidJson(err)));
    //     }
    // }

    // TODO:
    // if args.body_type.url_encoded {
    //     concatenated_body.push('&' as u8);
    // }

    let request = request.build()?;
    let mut response = client.execute(request)?;
    let status = response.status();

    if atty::is(atty::Stream::Stderr) {
        colored::control::set_override(true);
    }
    eprintln!(
        "{}",
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

    if args.print_headers {
        for (key, value) in response.headers().iter() {
            eprintln!(
                "{}{}{}",
                key.to_string().yellow().bold(),
                "=".dimmed(),
                value.to_str()?
            );
        }
    }
    colored::control::set_override(false);

    let response_is_json = response.headers().iter().any(|(key, value)| {
        key == CONTENT_TYPE
            && value
                .to_str()
                .is_ok_and(|value| value.contains("application/json"))
    });

    let mut bytes = Vec::new();
    response.read_to_end(&mut bytes)?;
    if response_is_json {
        if atty::is(atty::Stream::Stdout) {
            colored::control::set_override(true);
        }
        let body = String::from_utf8(bytes)?;
        let body: serde_json::Value = serde_json5::from_str(&body)?;
        pretty_print(&body, 0);
        println!();
        colored::control::set_override(false);
    } else {
        std::io::stdout().write_all(&bytes)?;
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

use std::{
    io::{Read, Write},
    sync::Arc,
};

use base64::prelude::*;
use clap::{Args, Parser, ValueEnum, builder::styling};
use colored::*;
use reqwest::{
    blocking::{ClientBuilder, multipart::Form},
    header::{AUTHORIZATION, CONTENT_TYPE},
    redirect::Policy,
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

    /// Add a query parameter to the URL.
    #[arg(short = 'q', long = "query", value_name = "KEY=VALUE")]
    query_params: Vec<String>,

    /// Follow redirects
    #[arg(short = 'f', long = "follow")]
    follow_redirects: bool,

    /// Maximum number of redirects to follow
    #[arg(long = "max-redirs", default_value = "10")]
    max_redirects: usize,

    /// Print headers
    #[arg(long = "print-headers")]
    print_headers: bool,

    #[command(flatten)]
    auth_type: AuthType,

    /// Add body contents (prefix with @ to read from file).
    #[arg(short = 'b', long = "body")]
    bodies: Vec<String>,

    #[command(flatten)]
    body_type: BodyType,
}

#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
struct AuthType {
    /// Shorthand notation for the `Authorization` header.
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
    #[arg(short = 'J', long = "json-body")]
    json: bool,

    /// The body is URL encoded.
    /// Sets the `content-type=application/x-www-form-urlencoded` header.
    /// Multiple bodies are concatenated with a `&` between them.
    #[arg(short = 'U', long = "url-body")]
    url_encoded: bool,

    /// The body is a multipart form.
    /// Sets the `content-type=multipart/form-data` header.
    /// Multiple occurrences are allowed.
    #[arg(short = 'F', long = "form-body")]
    form: bool,
}

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

fn main() {
    if let Err(error) = run() {
        eprintln!("{} {error}", "Error:".red());
        std::process::exit(1);
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
    let args = Cli::parse();

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

    let redirect_policy = if args.follow_redirects {
        Policy::limited(args.max_redirects)
    } else {
        Policy::none()
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

    if let Some(auth) = args.auth_type.auth {
        request = request.header(AUTHORIZATION, auth);
    }

    if let Some(user) = args.auth_type.user {
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

    if args.body_type.url_encoded {
        request = request.header(CONTENT_TYPE, "application/x-www-form-urlencoded");
    }
    if args.body_type.json {
        request = request.header(CONTENT_TYPE, "application/json");
    }

    let mut bodies: Vec<BodyContent> = Vec::new();
    for body in args.bodies {
        bodies.push(if let Some(file_path) = body.strip_prefix('@') {
            let mut file = std::fs::File::open(file_path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            BodyContent::Binary(buffer)
        } else {
            BodyContent::String(body)
        });
    }

    if args.body_type.form {
        let mut form = Form::new();
        for field in bodies {
            let field = field.to_string()?;
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| Error::InvalidFormField(field.clone()))?;
            form = form.text(key.to_string(), value.to_string());
        }
        request = request.multipart(form);
    } else {
        let mut concatenated_body: Vec<u8> = Vec::new();
        for body in bodies {
            if args.body_type.json {
                let body = body.clone().to_string()?;
                if let Err(err) = serde_json5::from_str::<serde_json::Value>(&body) {
                    return Err(Box::new(Error::InvalidJson(err)));
                }
            }
            if args.body_type.url_encoded {
                concatenated_body.push('&' as u8);
            }
            concatenated_body.append(&mut body.to_bytes());
        }
        request = request.body(concatenated_body);
    }

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

fn styles() -> styling::Styles {
    use styling::{AnsiColor, Style};

    styling::Styles::styled()
        .header(Style::new().bold().fg_color(Some(AnsiColor::Yellow.into())))
        .usage(Style::new().bold().fg_color(Some(AnsiColor::Yellow.into())))
        .literal(Style::new().fg_color(Some(AnsiColor::Green.into())))
        .placeholder(Style::new().fg_color(Some(AnsiColor::Cyan.into())))
        .error(Style::new().bold().fg_color(Some(AnsiColor::Red.into())))
        .valid(Style::new().bold().fg_color(Some(AnsiColor::Green.into())))
        .invalid(Style::new().bold().fg_color(Some(AnsiColor::Red.into())))
}

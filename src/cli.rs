use clap::{Args, Parser, ValueEnum, builder::styling};

pub const STDIN: &str = "-";

#[derive(Parser, Debug)]
#[command(version, about)]
#[command(styles = styles())]
pub struct Cli {
    /// The URL to request.
    pub url: String,

    #[arg(short, long, default_value_t = Method::GET)]
    pub method: Method,

    /// Add a header to the request.
    #[arg(short = 'H', long = "header", value_name = "NAME=VALUE")]
    pub headers: Vec<String>,

    /// Add a cookie to the request.
    #[arg(short = 'c', long = "cookie", value_name = "NAME=VALUE")]
    pub cookies: Vec<String>,

    /// Add a query parameter to the URL.
    #[arg(short = 'q', long = "query", value_name = "KEY=VALUE")]
    pub query_params: Vec<String>,

    /// Do not follow redirects
    #[arg(long = "no-follow")]
    pub no_follow_redirects: bool,

    /// Maximum number of redirects to follow
    #[arg(long = "max-redirs", default_value = "10")]
    pub max_redirects: usize,

    /// Print headers
    #[arg(long = "print-headers")]
    pub print_headers: bool,

    #[command(flatten)]
    pub auth_method: AuthMethod,

    #[command(flatten)]
    pub body_source: BodySource,

    /// The body is JSON.
    /// Sets the `content-type=application/json` header.
    /// Denies the request if the body is syntactically malformed.
    #[arg(short = 'j', long = "json-body")]
    pub json: bool,

    /// The body is URL encoded.
    /// Sets the `content-type=application/x-www-form-urlencoded` header.
    /// Multiple bodies are concatenated with a `&` between them.
    #[arg(long = "url-encoded-body")]
    pub url_encoded_body: bool,
}

#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
pub struct AuthMethod {
    /// Shorthand notation for the `Authorization` header.
    #[arg(short = 'a', long = "auth")]
    pub auth: Option<String>,

    /// HTTP Basic Authentication in the format username:password.
    /// If password is omitted, you will be prompted for it.
    #[arg(long = "user", value_name = "USER[:PASSWORD]")]
    pub user: Option<String>,
}

// TODO: Use enum?
#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
pub struct BodySource {
    /// Add body contents
    #[arg(short = 'b', long = "body")]
    pub string: Option<String>,

    /// Read body contents from file (- for stdin)
    #[arg(short = 'i', long = "input")]
    pub path: Option<std::path::PathBuf>,

    /// Add multipart form body.
    /// Sets the `content-type=multipart/form-data` header.
    #[arg(short = 'F', long = "form-field")]
    pub form_fields: Option<Vec<String>>,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum Method {
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

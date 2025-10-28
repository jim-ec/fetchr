# fetchr

A simple CLI tool to make HTTP requests.

This project aims to be a Rust-based implementation of a CLI tool similar to curl.

You can specify a URL, query parameters, body, headers, cookies, etc., via CLI options,
and the tool prints the HTTP response code and body to the terminal.

## Install

Currently, the tool is only installable via `cargo`:

```sh
cargo install fetchr
```

Run the same command to update the tool.

## Examples

- `fetchr <url>`: A GET request
- `fetchr <url> -m post -b 'hello there!'`: A POST request with text payload
- `fetchr <url> -m post -b '{text: "general kenobi!"}' --json-body`: A POST request with JSON payload. The JSON content is parsed and validated before dispatching the request.
- `fetchr <url> -H 'Authorization=Bearer 12345'`: A request with headers
- `fetchr <url> -a 'Bearer 12345'`: Shorthand notation for an authorization header
- `fetchr <url> -m patch -F name=obiwan -F occupation=jedi`: A PATCH request with a multipart form-data body
- `fetchr <url> --print-headers`: Also print response headers

There are more options, just run `fetchr -h`.

At this point, the README documentation is deliberately vague because the options might change, features be added or modified, etc. This will of course not be the case when this tool reaches `1.0`.

If you need something this tool cannot do yet, please file an issue or PR. I myself am developing fetchr while I am using it at work.

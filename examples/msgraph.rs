//!
//! This example showcases the Microsoft Graph OAuth2 process for requesting access to Microsoft
//! services using PKCE.
//!
//! Before running it, you'll need to generate your own Microsoft OAuth2 credentials.
//!
//! * Go to https://apps.dev.microsoft.com/
//! * Click Add an App.
//! * Skip the guided setup.
//! * Set Web Redirect URL to http://localhost:3003/redirect
//! * Add Delegated Permissions for Files.Read
//! * Use the Application Id as MSGRAPH_CLIENT_ID.
//! * Click Generate New Password.
//! * Copy the password and use it as MSGRAPH_CLIENT_SECRET.
//!
//! In order to run the example call:
//!
//! ```sh
//! MSGRAPH_CLIENT_ID=xxx MSGRAPH_CLIENT_SECRET=yyy cargo run --example msgraph
//! ```
//!
//! ...and follow the instructions.
//!

extern crate base64;
extern crate oauth2;
extern crate rand;
extern crate url;

use oauth2::basic::BasicClient;
// Alternatively, this can be `oauth2::curl::http_client` or a custom client.
use oauth2::reqwest::http_client;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenUrl,
};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use url::Url;

fn main() {
    let graph_client_id = ClientId::new(
        env::var("MSGRAPH_CLIENT_ID").expect("Missing the MSGRAPH_CLIENT_ID environment variable."),
    );
    let graph_client_secret = ClientSecret::new(
        env::var("MSGRAPH_CLIENT_SECRET")
            .expect("Missing the MSGRAPH_CLIENT_SECRET environment variable."),
    );
    let auth_url = AuthUrl::new(
        Url::parse("https://login.microsoftonline.com/common/oauth2/v2.0/authorize")
            .expect("Invalid authorization endpoint URL"),
    );
    let token_url = TokenUrl::new(
        Url::parse("https://login.microsoftonline.com/common/oauth2/v2.0/token")
            .expect("Invalid token endpoint URL"),
    );

    // Set up the config for the Microsoft Graph OAuth2 process.
    let client = BasicClient::new(
        graph_client_id,
        Some(graph_client_secret),
        auth_url,
        Some(token_url),
    )
    // Microsoft Graph requires client_id and client_secret in URL rather than
    // using Basic authentication.
    .set_auth_type(AuthType::RequestBody)
    // This example will be running its own server at localhost:3003.
    // See below for the server implementation.
    .set_redirect_url(RedirectUrl::new(
        Url::parse("http://localhost:3003/redirect").expect("Invalid redirect URL"),
    ));

    // Microsoft Graph supports Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the authorization URL to which we'll redirect the user.
    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        // This example requests read access to OneDrive.
        .add_scope(Scope::new(
            "https://graph.microsoft.com/Files.Read".to_string(),
        ))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    println!(
        "Open this URL in your browser:\n{}\n",
        authorize_url.to_string()
    );

    // A very naive implementation of the redirect server.
    let listener = TcpListener::bind("127.0.0.1:3003").unwrap();
    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let code;
            let state;
            {
                let mut reader = BufReader::new(&stream);

                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();

                let redirect_url = request_line.split_whitespace().nth(1).unwrap();
                let url = Url::parse(&("http://localhost".to_string() + redirect_url)).unwrap();

                let code_pair = url
                    .query_pairs()
                    .find(|pair| {
                        let &(ref key, _) = pair;
                        key == "code"
                    })
                    .unwrap();

                let (_, value) = code_pair;
                code = AuthorizationCode::new(value.into_owned());

                let state_pair = url
                    .query_pairs()
                    .find(|pair| {
                        let &(ref key, _) = pair;
                        key == "state"
                    })
                    .unwrap();

                let (_, value) = state_pair;
                state = CsrfToken::new(value.into_owned());
            }

            let message = "Go back to your terminal :)";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
                message.len(),
                message
            );
            stream.write_all(response.as_bytes()).unwrap();

            println!("MS Graph returned the following code:\n{}\n", code.secret());
            println!(
                "MS Graph returned the following state:\n{} (expected `{}`)\n",
                state.secret(),
                csrf_state.secret()
            );

            // Exchange the code with a token.
            let token = client
                .exchange_code(code)
                // Send the PKCE code verifier in the token request
                .set_pkce_verifier(pkce_code_verifier)
                .request(http_client);

            println!("MS Graph returned the following token:\n{:?}\n", token);

            // The server will terminate itself after collecting the first code.
            break;
        }
    }
}

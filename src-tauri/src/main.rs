#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::{command, generate_handler, AppHandle, Runtime};

mod db;
mod error;
mod plugin;
mod s3;

fn main() {
    println!("{}", tauri::path::BaseDirectory::AppData.variable());

    let mut app = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(plugin::Api::init("sqlite:boom.db").build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.run(|_, _| {})
    // .run(tauri::generate_context!())
    // .expect("error while running tauri application");
}

// mod server {
//     use std::net::{SocketAddr};

//     use axum::{routing::{ Router, post }, response::IntoResponse, http::{StatusCode, Method}, extract::BodyStream};
//     use futures_util::StreamExt;
//     use tower_http::cors::{CorsLayer, Any};
//
//     pub async fn run() {
//         let cors = CorsLayer::new()
//             .allow_methods([Method::POST, Method::GET])
//             .allow_origin(Any)
//             .allow_headers(Any);

//         println!("Starting server");
//
//         let r = Router::new()
//             .route("/api/stream", post(stream))
//             .layer(cors);

//         let addr = SocketAddr::from(([127, 0, 0, 1], 42069));
//         eprintln!("Listening on {}", addr);
//
//         axum::Server::bind(&addr)
//         .serve(r.into_make_service()).await.unwrap();
//     }

//     async fn stream(mut stream: BodyStream) -> impl IntoResponse {
//         while let Some(chunk) = stream.next().await {
//             let chunk = chunk.unwrap();
//             println!("chunk: {:?}", chunk);
//         }
//
//     }
// }

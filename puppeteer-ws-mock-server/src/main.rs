use std::time::{Duration, Instant};

use actix_web::{
    App, Error, HttpRequest, HttpResponse, HttpServer,
    rt::{self, time::interval},
    web,
};
use actix_ws::{AggregatedMessage, Message};
use futures_util::StreamExt;
use puppeteer::{PuppetEvent, PuppetFrame};

async fn echo(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let (res, mut session, stream) = actix_ws::handle(&req, stream)?;

    let mut stream = stream
        .aggregate_continuations()
        // aggregate continuation frames up to 1MiB
        .max_continuation_size(2_usize.pow(20));

    // start task but don't wait for it
    rt::spawn(async move {
        // receive messages from websocket
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(AggregatedMessage::Text(text)) => {
                    // echo text message
                    session.text(text).await.unwrap();
                }

                Ok(AggregatedMessage::Binary(bin)) => {
                    // echo binary message
                    session.binary(bin).await.unwrap();
                }

                Ok(AggregatedMessage::Ping(msg)) => {
                    // respond to PING frame with PONG frame
                    session.pong(&msg).await.unwrap();
                }

                _ => {}
            }
        }
    });

    // respond immediately with response connected to WS session
    Ok(res)
}

async fn mock_server(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let (res, mut session, mut stream) = actix_ws::handle(&req, stream)?;

    let beginning = Instant::now();

    let player_list: Vec<String> = vec!["arte".into(), "rawe".into(), "qicg".into(), "R3AL".into()];

    // changing camera thread
    rt::spawn(async move {
        // load map
        {
            let message = PuppetEvent::MapChange {
                game_mod: "cstrike".into(),
                map_name: "bkz_goldbhop".into(),
            };

            let message = message.encode_message_msgpack().unwrap();

            session.binary(message).await.unwrap();
        }

        // send player list
        {
            let message = PuppetEvent::PlayerList(player_list.clone())
                .encode_message_msgpack()
                .unwrap();

            session.binary(message).await.unwrap();
        }

        const UPDATE_RATE: f32 = 0.02;

        let mut update_interval = interval(Duration::from_secs_f32(UPDATE_RATE));

        loop {
            let now = Instant::now();

            // let mut handle_message = async |s: String| {
            //     println!("recived message `{}`", s);
            //     if let Some(_s) = s.strip_prefix(REQUEST_PLAYER_LIST) {
            //         let message = PuppetEvent::PlayerList(player_list.clone());

            //         let test = message.encode_message_json().unwrap();
            //         println!("{}", test);
            //         let message = message.encode_message_msgpack().unwrap();

            //         println!("received player list message");

            //         match session.binary(message).await {
            //             Ok(_) => {}
            //             Err(_) => {}
            //         }
            //     }
            // };

            tokio::select! {
                _ = update_interval.tick() => {
                    let value = (now.duration_since(beginning).as_secs_f32() * 10.) % 360.;

                    let frame: Vec<PuppetFrame> = player_list.iter().map(|curr_player| {
                        let viewangles = match curr_player.as_str() {
                            "arte" => [0., value, 0.],
                            "rawe" => [(value - (-90.)).rem_euclid(180. + 1.) + -90. , 0., 0.],
                            "qicg" => [(value - (-90.)).rem_euclid(180. + 1.) + -90., value, 0.],
                            _ => [0f32; 3],
                        };

                        let vieworg = match curr_player.as_str() {
                            "R3AL" => [0., 0., value],
                            _ => [0f32; 3],
                        };

                        PuppetFrame {
                            vieworg,
                            viewangles,
                            timer_time: 0.,
                        }

                    }).collect();

                    let message = PuppetEvent::PuppetFrame { server_time: 0., frame };

                    let message = message.encode_message_msgpack().unwrap();

                    // need to handle so that terminated connection doesn't panic thread
                    match session.binary(message).await {
                        Ok(_) => {}
                        Err(_) => {
                            break;
                        }
                    }

                },
                msg = stream.next() => {
                    println!("recived some message `{:?}`", msg);
                    // msg must be text
                    match msg {
                        Some(Ok(msg)) => {
                            println!("recived ok `{:?}`", msg);

                            // no more command from client to server
                            match msg {
                                // Message::Text(byte_string) => {
                                //     handle_message(byte_string.to_string()).await;
                                // },
                                _ => ()
                            }
                        },
                        Some(Err(_err)) => {
                            break;
                        },
                        None => {
                            break;
                        },
                    }
                }
            };
        }
    });

    Ok(res)
}

#[actix_web::main]
#[cfg(not(target_arch = "wasm32"))]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/echo", web::get().to(echo))
            .route("/mock-server", web::get().to(mock_server))
    })
    .bind(("127.0.0.1", 3002))?
    .run()
    .await
}

#[cfg(target_arch = "wasm32")]
fn main() {}

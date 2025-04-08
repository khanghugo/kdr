use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use loader::{ResourceIdentifier, native::NativeResourceProvider};
use tracing::{info, info_span, warn};
use uuid::Uuid;

use crate::{
    send_res::{gchimp_resmake_way, native_way},
    utils::sanitize_identifier,
};

#[derive(Debug, Clone)]
// The state doesn't change after starting the server so this works nicely.
struct AppData {
    resource_provider: NativeResourceProvider,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

// must be a POST request
#[post("request-map")]
async fn request_map(
    req: web::Json<ResourceIdentifier>,
    data: web::Data<AppData>,
) -> impl Responder {
    let map_name = &req.map_name;
    let game_mod = &req.game_mod;

    let _span = info_span!("request", request_id = %Uuid::new_v4()).entered();
    info!("Request identifier: {:?}", req);

    if map_name.is_empty() {
        info!("Request has no map name");
        return HttpResponse::BadRequest().body("No map name provided.");
    }

    if game_mod.is_empty() {
        info!("Request has no game mod");
        return HttpResponse::BadRequest().body("No game mod provided.");
    }

    let Some(sanitized_identifier) = sanitize_identifier(&req) else {
        info!("Request fails sanitizer");
        return HttpResponse::BadRequest().body("Invalid resource identifier.");
    };

    match native_way(&sanitized_identifier, &data.resource_provider).await {
        Ok(bytes) => {
            let file_name = sanitized_identifier.map_name.replace(".bsp", ".zip");

            info!("Successful request");

            return HttpResponse::Ok()
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{file_name}\""),
                ))
                .body(bytes);
        }
        Err(err) => {
            warn!("Request failed: {}", err);
            return HttpResponse::InternalServerError().body("Cannot find requested map.");
        }
    };
}

async fn manual_hello(data: web::Data<AppData>) -> impl Responder {
    HttpResponse::Ok().body(format!(
        "Hey there! {} ",
        data.resource_provider.game_dir.display()
    ))
}

#[actix_web::main]
pub async fn start_server(
    resource_provider: NativeResourceProvider,
    port: u16,
) -> std::io::Result<()> {
    let data = AppData { resource_provider };

    HttpServer::new(move || {
        #[cfg(feature = "cors")]
        let cors = actix_cors::Cors::permissive();

        let app = App::new()
            .service(hello)
            .service(echo)
            .service(request_map)
            .route("/hey", web::get().to(manual_hello))
            .app_data(web::Data::new(data.clone()));

        #[cfg(feature = "cors")]
        let app = app.wrap(cors);

        app
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}

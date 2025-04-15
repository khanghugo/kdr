use std::path::PathBuf;

use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use common::CANNOT_FIND_REQUESTED_MAP_ERROR;
use loader::{MapList, ResourceIdentifier, ResourceProvider, native::NativeResourceProvider};
use pollster::FutureExt;
use tracing::{info, info_span, warn};
use uuid::Uuid;

use crate::{
    ServerArgs,
    send_res::{gchimp_resmake_way, native_way},
    utils::sanitize_identifier,
};

#[derive(Debug, Clone)]
// The state doesn't change after starting the server so this works nicely.
struct AppData {
    resource_provider: NativeResourceProvider,
    // .zip file already loaded into memory
    // optional to make sure that we have a file to distribute
    common_resource: Option<PathBuf>,
    map_list: MapList,
}

#[get("/request-common")]
async fn request_common_resource(data: web::Data<AppData>) -> impl Responder {
    info!("Request common resource");

    if let Some(path) = data.common_resource.clone() {
        match std::fs::read(path.as_path()) {
            Ok(bytes) => HttpResponse::Ok()
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"common.zip\""),
                ))
                .body(bytes),
            Err(err) => {
                warn!("Cannot read common resource `{}`: {}", path.display(), err);

                HttpResponse::InternalServerError().finish()
            }
        }
    } else {
        HttpResponse::NoContent().finish()
    }
}

// must be a POST request
#[post("/request-map")]
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
            return HttpResponse::NotFound().body(CANNOT_FIND_REQUESTED_MAP_ERROR);
        }
    };
}

#[get("/request-map-list")]
async fn request_map_list(data: web::Data<AppData>) -> impl Responder {
    HttpResponse::Ok().json(&data.map_list)
}

#[actix_web::main]
pub async fn start_server(args: ServerArgs) -> std::io::Result<()> {
    let ServerArgs {
        resource_provider,
        port,
        common_resource,
    } = args;

    let map_list = resource_provider
        .get_map_list()
        .block_on()
        .expect("cannot get map list");

    let data = AppData {
        resource_provider,
        common_resource,
        map_list,
    };

    info!("Staring kdr API server");
    info!(
        "Resource provider game directory: {}",
        data.resource_provider.game_dir.display()
    );

    if let Some(common_resource_path) = &data.common_resource {
        info!("Common resourch path: {}", common_resource_path.display());
    } else {
        info!("No common resource provided");
    }

    HttpServer::new(move || {
        #[cfg(feature = "cors")]
        let cors = actix_cors::Cors::permissive();

        let app = App::new()
            .service(request_map)
            .service(request_common_resource)
            .service(request_map_list)
            .app_data(web::Data::new(data.clone()));

        #[cfg(feature = "cors")]
        let app = app.wrap(cors);

        app
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

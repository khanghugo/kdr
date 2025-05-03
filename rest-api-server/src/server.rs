use std::sync::{Arc, RwLock};

use actix_web::{App, HttpResponse, HttpServer, Responder, get, middleware::Compress, post, web};
use common::{CANNOT_FIND_REQUESTED_MAP_ERROR, CANNOT_FIND_REQUESTED_REPLAY_ERR};
use config::KDRApiServerConfig;
use loader::{MapIdentifier, MapList, ReplayList, native::NativeResourceProvider};
use serde::Deserialize;
use tracing::{info, info_span, warn};
use uuid::Uuid;

use crate::{
    ServerArgs,
    send_res::{gchimp_resmake_way, native_way},
    utils::{
        create_common_resource, get_map_list, get_replay, get_replay_list, sanitize_identifier,
    },
};

#[derive(Debug, Clone)]
// The state doesn't change after starting the server so this works nicely.
struct AppData {
    resource_provider: NativeResourceProvider,
    // .zip file already loaded onto memory
    common_resource: Option<Vec<u8>>,
    map_list: Arc<RwLock<MapList>>,
    replay_list: Arc<RwLock<ReplayList>>,

    // the rest of the config
    config: KDRApiServerConfig,
}

#[get("/request-common")]
async fn request_common_resource(data: web::Data<AppData>) -> impl Responder {
    info!("Request common resource");

    if let Some(bytes) = &data.common_resource {
        HttpResponse::Ok()
            .content_type("application/zip")
            .append_header(("Content-Transfer-Encoding", "binary"))
            .append_header(("Content-Length", bytes.len()))
            .append_header((
                "Content-Disposition",
                format!("attachment; filename=\"common.zip\""),
            ))
            .body(bytes.clone())
    } else {
        HttpResponse::NoContent().finish()
    }
}

// must be a POST request
#[post("/request-map")]
async fn request_map(req: web::Json<MapIdentifier>, data: web::Data<AppData>) -> impl Responder {
    let map_name = &req.map_name;
    let game_mod = &req.game_mod;

    let _span = info_span!("resource request", request_id = %Uuid::new_v4()).entered();
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

    let bytes = if data.config.use_resmake_zip {
        gchimp_resmake_way(&sanitized_identifier, &data.resource_provider).await
    } else {
        native_way(&sanitized_identifier, &data.resource_provider).await
    };

    match bytes {
        Ok(bytes) => {
            let file_name = sanitized_identifier.map_name.replace(".bsp", ".zip");

            info!("Successful request");

            return HttpResponse::Ok()
                .content_type("application/zip")
                .append_header(("Content-Transfer-Encoding", "binary"))
                .append_header(("Content-Length", bytes.len()))
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

#[derive(Debug, Deserialize)]
struct ReplayRequest {
    replay_name: String,
}

#[post("/request-replay")]
async fn request_replay(req: web::Json<ReplayRequest>, data: web::Data<AppData>) -> impl Responder {
    let replay_name = &req.replay_name;

    let _span = info_span!("replay request", request_id = %Uuid::new_v4()).entered();
    info!("Replay request: {:?}", req);

    if replay_name.is_empty() {
        info!("Request has no replay name");
        return HttpResponse::BadRequest().body("No replay provided.");
    }

    let Some(replay_blob) = get_replay(&data.config, replay_name) else {
        warn!("Cannot get replay: `{}`", replay_name);

        return HttpResponse::NotFound().body(CANNOT_FIND_REQUESTED_REPLAY_ERR);
    };

    let buf = rmp_serde::to_vec(&replay_blob).unwrap();

    HttpResponse::Ok().body(buf)
}

#[derive(Deserialize)]
struct UpdateRequest {
    secret: String,
}

#[get("/request-map-list")]
async fn request_map_list(data: web::Data<AppData>) -> impl Responder {
    info!("Request map list");

    HttpResponse::Ok().json(&*data.map_list.read().unwrap())
}

#[get("/request-replay-list")]
async fn request_replay_list(data: web::Data<AppData>) -> impl Responder {
    info!("Request replay list");

    HttpResponse::Ok().json(&*data.replay_list.read().unwrap())
}

#[post("/update-map-list")]
async fn update_map_list(
    req: web::Json<UpdateRequest>,
    data: web::Data<AppData>,
) -> impl Responder {
    let input_secret = &req.secret;

    if input_secret == &data.config.secret {
        let new_map_list = get_map_list(&data.resource_provider).await;

        match data.map_list.write() {
            Ok(mut lock) => {
                *lock = new_map_list;
                HttpResponse::Ok().finish()
            }
            Err(_) => HttpResponse::InternalServerError().finish(),
        }
    } else {
        HttpResponse::Forbidden().finish()
    }
}

#[post("/update-replay-list")]
async fn update_replay_list(
    req: web::Json<UpdateRequest>,
    data: web::Data<AppData>,
) -> impl Responder {
    let input_secret = &req.secret;

    if input_secret == &data.config.secret {
        let new_replay_list = get_replay_list(&data.config).await;

        match data.replay_list.write() {
            Ok(mut lock) => {
                *lock = new_replay_list;
                HttpResponse::Ok().finish()
            }
            Err(_) => HttpResponse::InternalServerError().finish(),
        }
    } else {
        HttpResponse::Forbidden().finish()
    }
}

#[actix_web::main]
pub async fn start_server(args: ServerArgs) -> std::io::Result<()> {
    let ServerArgs { config } = args;

    let game_dir = &config.game_dir;
    let port = config.port;
    let resource_provider = NativeResourceProvider::new(game_dir.as_path());

    let common_resource = if config.common_resource.is_empty() {
        info!("No common resource given");
        None
    } else {
        info!(
            "Found ({}) common resources given. Creating .zip for common resources",
            config.common_resource.len()
        );
        create_common_resource(game_dir.as_path(), &config.common_resource).into()
    };

    let map_list = get_map_list(&resource_provider).await;
    let replay_list = get_replay_list(&config).await;

    let use_resmake_zip = config.use_resmake_zip;

    let data = AppData {
        resource_provider,
        common_resource,
        map_list: Arc::new(RwLock::new(map_list)),
        replay_list: Arc::new(RwLock::new(replay_list)),
        config,
    };

    info!("Staring kdr API server");
    info!(
        "Resource provider game directory: {}",
        data.resource_provider.game_dir.display()
    );

    if use_resmake_zip {
        info!(
            "Using gchimp ResMake option. This will only send zip files of maps in the \"maps\" folder"
        );
    } else {
        info!(
            "Using native resource fetching. This will search for the entire game directory for every resource request"
        )
    }

    HttpServer::new(move || {
        #[cfg(feature = "cors")]
        let cors = actix_cors::Cors::permissive();

        let app = App::new()
            // enable compression
            .wrap(Compress::default())
            // apis
            .service(request_map)
            .service(request_replay)
            .service(request_common_resource)
            .service(request_map_list)
            .service(request_replay_list)
            .service(update_map_list)
            .service(update_replay_list)
            .app_data(web::Data::new(data.clone()));

        #[cfg(feature = "cors")]
        let app = app.wrap(cors);

        app
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

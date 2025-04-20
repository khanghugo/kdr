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
    // .zip file already loaded onto memory
    common_resource: Option<Vec<u8>>,
    map_list: MapList,
    use_resmake_zip: bool,
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

    let bytes = if data.use_resmake_zip {
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
        use_resmake_zip,
    } = args;

    let map_list = resource_provider
        .get_map_list()
        .block_on()
        .expect("cannot get map list");

    let data = AppData {
        resource_provider,
        common_resource,
        map_list,
        use_resmake_zip,
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

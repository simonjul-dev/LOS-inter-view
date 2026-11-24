mod los;
mod terrain;

use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use terrain::Terrain;
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    terrain: Arc<Mutex<Terrain>>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ApiPoint {
    longitude: f64,
    latitude: f64,
    hauteur: f64,
}

impl From<ApiPoint> for los::PointObservation {
    fn from(point: ApiPoint) -> Self {
        Self {
            longitude: point.longitude,
            latitude: point.latitude,
            hauteur: point.hauteur,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ViewshedRequest {
    longitude: f64,
    latitude: f64,
    hauteur_observateur: f64,
    rayon_metres: f64,
}

#[derive(Debug, Deserialize)]
struct LosRequest {
    point_a: ApiPoint,
    point_b: ApiPoint,
}

#[derive(Debug, Deserialize)]
struct RouteRequest {
    points: Vec<ApiPoint>,
}

fn viewshed_to_json(resultat: &los::ResultatViewshed) -> Value {
    json!({
        "longitude": resultat.longitude,
        "latitude": resultat.latitude,
        "hauteur_observateur": resultat.hauteur_observateur,
        "rayon_metres": resultat.rayon_metres,
        "largeur": resultat.largeur,
        "hauteur": resultat.hauteur,
        "pixels_visibles": resultat.pixels_visibles(),
        "visibilite": resultat.visibilite,
        "geotransform": resultat.geotransform,
        "no_data": resultat.no_data
    })
}

async fn viewshed_api_get(
    State(etat): State<AppState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let requete = ViewshedRequest {
        longitude: 5.5,
        latitude: 43.5,
        hauteur_observateur: 100.0,
        rayon_metres: 10_000.0,
    };
    calculer_viewshed(etat, requete).await
}

async fn viewshed_api_post(
    State(etat): State<AppState>,
    Json(requete): Json<ViewshedRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    calculer_viewshed(etat, requete).await
}

async fn calculer_viewshed(
    etat: AppState,
    requete: ViewshedRequest,
) -> Result<Json<Value>, (StatusCode, String)> {
    if !requete.longitude.is_finite()
        || !requete.latitude.is_finite()
        || !requete.hauteur_observateur.is_finite()
        || !requete.rayon_metres.is_finite()
        || !(-180.0..=180.0).contains(&requete.longitude)
        || !(-90.0..=90.0).contains(&requete.latitude)
    {
        return Err((StatusCode::BAD_REQUEST, "Paramètres numériques invalides".into()));
    }

    let terrain = etat.terrain.lock().await;
    let observateur = los::PointObservation {
        longitude: requete.longitude,
        latitude: requete.latitude,
        hauteur: requete.hauteur_observateur,
    };

    let resultat = los::analyser_viewshed(&terrain, observateur, requete.rayon_metres)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(viewshed_to_json(&resultat)))
}

async fn los_api(
    State(etat): State<AppState>,
    Json(requete): Json<LosRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let terrain = etat.terrain.lock().await;
    let resultat = los::analyser_los(
        &terrain,
        requete.point_a.into(),
        requete.point_b.into(),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(json!({
        "visible": resultat.visible,
        "distance_metres": resultat.distance_metres,
        "angle_vertical": resultat.angle_vertical
    })))
}

async fn route_api(
    State(etat): State<AppState>,
    Json(requete): Json<RouteRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let terrain = etat.terrain.lock().await;
    let points: Vec<los::PointObservation> =
        requete.points.into_iter().map(Into::into).collect();

    let resultat = los::analyser_route(&terrain, &points)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let segments: Vec<Value> = resultat
        .segments
        .iter()
        .map(|segment| {
            json!({
                "visible": segment.visible,
                "distance_metres": segment.distance_metres,
                "angle_vertical": segment.angle_vertical
            })
        })
        .collect();

    Ok(Json(json!({
        "segments": segments,
        "distance_totale_metres": resultat.distance_totale_metres,
        "route_entierement_visible": resultat.route_entierement_visible
    })))
}

#[tokio::main]
async fn main() {
    let chemin_dted = r"D:\Julien\Data\Donnes Geographiques\DVD Données Demo Sirina SPMR2\MNT\DTED 1\e005\N43.DT1";

    println!("Chargement du terrain DTED...");
    let terrain = match Terrain::ouvrir(chemin_dted) {
        Ok(terrain) => terrain,
        Err(e) => {
            eprintln!("Erreur chargement DTED : {}", e);
            return;
        }
    };

    let (largeur, hauteur) = terrain.dimensions();
    println!("DTED charge avec succes : {} x {} pixels", largeur, hauteur);

    match terrain.resolution_metres() {
        Ok((resolution_x, resolution_y)) => {
            println!("Resolution approximative : {:.2} m E-W, {:.2} m N-S",
                resolution_x, resolution_y);
        }
        Err(e) => eprintln!("Erreur resolution : {}", e),
    }

    match terrain.limites() {
        Ok((lon_min, lat_min, lon_max, lat_max)) => {
            println!(
                "Emprise DTED : longitude {:.6} a {:.6}, latitude {:.6} a {:.6}",
                lon_min, lon_max, lat_min, lat_max
            );
        }
        Err(e) => eprintln!("Erreur limites : {}", e),
    }

    // Petit contrôle initial des calculs déjà présents dans le module LOS.
    let point_a = los::PointObservation {
        longitude: 5.5,
        latitude: 43.5,
        hauteur: 2.0,
    };
    let point_b = los::PointObservation {
        longitude: 5.6,
        latitude: 43.5,
        hauteur: 2.0,
    };
    let point_c = los::PointObservation {
        longitude: 5.7,
        latitude: 43.5,
        hauteur: 2.0,
    };

    match los::analyser_los(&terrain, point_a, point_b) {
        Ok(resultat) => println!(
            "Test LOS : visible={}, distance={:.2} m",
            resultat.visible, resultat.distance_metres
        ),
        Err(e) => eprintln!("Test LOS impossible : {}", e),
    }

    match los::analyser_route(&terrain, &[point_a, point_b, point_c]) {
        Ok(resultat) => println!(
            "Test route : {} segments, distance={:.2} m, entièrement visible={}",
            resultat.segments.len(),
            resultat.distance_totale_metres,
            resultat.route_entierement_visible
        ),
        Err(e) => eprintln!("Test route impossible : {}", e),
    }

    let etat = AppState {
        terrain: Arc::new(Mutex::new(terrain)),
    };

    let app = Router::new()
        .route("/api/viewshed", get(viewshed_api_get).post(viewshed_api_post))
        .route("/api/los", axum::routing::post(los_api))
        .route("/api/route", axum::routing::post(route_api))
        .fallback_service(ServeDir::new("web"))
        .with_state(etat);

    let adresse = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(adresse)
        .await
        .expect("Impossible de demarrer le serveur");

    println!("Serveur LOS disponible sur http://{}", adresse);
    println!("API viewshed : POST /api/viewshed");
    println!("API intervisibilite : POST /api/los");
    println!("API route : POST /api/route");

    axum::serve(listener, app)
        .await
        .expect("Erreur serveur");
}

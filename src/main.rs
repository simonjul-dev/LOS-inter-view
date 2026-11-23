mod los;
mod terrain;

use terrain::Terrain;

use axum::Router;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let chemin_dted = r"D:\Julien\Data\Donnes Geographiques\DVD Données Demo Sirina SPMR2\MNT\DTED 1\e005\N43.DT1";

    match Terrain::ouvrir(chemin_dted) {
        Ok(terrain) => {
            let (largeur, hauteur) = terrain.dimensions();
            match terrain.resolution_metres() {
                Ok((resolution_x, resolution_y)) => {
                    println!("Resolution DTED :");
                    println!("Longitude : {:.2} m", resolution_x);
                    println!("Latitude : {:.2} m", resolution_y);
                }
                Err(e) => println!("Erreur resolution : {}", e),
            }
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
            let points_route = vec![point_a, point_b, point_c];
            let observateur = los::PointObservation {
                longitude: 5.5,
                latitude: 43.5,
                hauteur: 100.0,
            };

            match los::analyser_viewshed(&terrain, observateur, 10_000.0) {
                Ok(resultat) => {
                    println!("--- RESULTAT VIEWSHED ---");
                    println!(
                        "Observateur : {:.6}, {:.6}",
                        resultat.longitude, resultat.latitude
                    );
                    println!("Hauteur : {:.2} m", resultat.hauteur_observateur);
                    println!("Rayon : {:.2} m", resultat.rayon_metres);
                    println!("Dimensions : {} x {}", resultat.largeur, resultat.hauteur);
                    println!("Taille de la grille : {}", resultat.visibilite.len());
                }
                Err(e) => println!("Erreur Viewshed : {}", e),
            }

            match los::analyser_route(&terrain, &points_route) {
                Ok(resultat) => {
                    println!("--- RESULTAT ANALYSE ROUTE ---");
                    println!("Nombre de segments : {}", resultat.segments.len());
                    println!("Distance totale : {:.2} m", resultat.distance_totale_metres);
                    println!(
                        "Route entièrement visible : {}",
                        resultat.route_entierement_visible
                    );

                    for (indice, segment) in resultat.segments.iter().enumerate() {
                        println!("Segment {} : visibilité = {}", indice + 1, segment.visible);
                    }
                }
                Err(e) => println!("Erreur analyse route : {}", e),
            }
            match los::analyser_los(&terrain, point_a, point_b) {
                Ok(resultat) => {
                    println!("--- RESULTAT ANALYSE LOS ---");
                    println!("Visible : {}", resultat.visible);
                    println!("Distance : {:.2} m", resultat.distance_metres);
                    println!("Angle vertical : {:.4} degres", resultat.angle_vertical);
                }
                Err(e) => println!("Erreur analyse LOS : {}", e),
            }
            match terrain.coordonnees_pixel(5.5, 43.5) {
                Ok((colonne, ligne)) => {
                    println!("Pixel correspondant : ({}, {})", colonne, ligne);
                }
                Err(e) => println!("Erreur coordonnées : {}", e),
            }
            match terrain.altitude_geo(5.5, 43.5) {
                Ok(altitude) => {
                    println!("Altitude geographique : {} m", altitude);
                }
                Err(e) => println!("Erreur altitude geographique : {}", e),
            }
            match terrain.limites() {
                Ok((lon_min, lat_min, lon_max, lat_max)) => {
                    println!("Limites geographiques :");
                    println!("Longitude : {} a {}", lon_min, lon_max);
                    println!("Latitude : {} a {}", lat_min, lat_max);
                }
                Err(e) => println!("Erreur limites : {}", e),
            }

            println!("DTED charge avec succes !");
            println!("Dimensions : {} x {}", largeur, hauteur);

            match terrain.altitude_pixel(600, 600) {
                Ok(altitude) => println!("Altitude au centre : {} m", altitude),
                Err(e) => println!("Erreur altitude : {}", e),
            }
        }
        Err(e) => println!("Erreur chargement DTED : {}", e),
    }
    let app = Router::new().fallback_service(ServeDir::new("web"));

    let adresse = "127.0.0.1:3000";

    let listener = tokio::net::TcpListener::bind(adresse)
        .await
        .expect("Impossible de demarrer le serveur");

    println!("Serveur LOS disponible sur http://{}", adresse);

    axum::serve(listener, app).await.expect("Erreur serveur");
}

use crate::terrain::Terrain;
use rayon::prelude::*;
/// Représente un point géographique utilisé pour l'analyse LOS.
#[derive(Debug, Clone, Copy)]
pub struct PointObservation {
    pub longitude: f64,
    pub latitude: f64,
    pub hauteur: f64,
}
/// Résultat d'une analyse de visibilité entre deux points.
#[derive(Debug)]
pub struct ResultatLOS {
    pub visible: bool,
    pub distance_metres: f64,
    pub angle_vertical: f64,
}

/// Résultat de l'analyse de visibilité sur une route.
#[derive(Debug)]
pub struct ResultatRoute {
    pub segments: Vec<ResultatLOS>,
    pub distance_totale_metres: f64,
    pub route_entierement_visible: bool,
}

/// Résultat d'une analyse de visibilité depuis un observateur.
#[derive(Debug, Clone)]
pub struct ResultatViewshed {
    pub longitude: f64,
    pub latitude: f64,
    pub hauteur_observateur: f64,
    pub rayon_metres: f64,
    pub largeur: usize,
    pub hauteur: usize,
    pub visibilite: Vec<u8>,
    pub geotransform: [f64; 6],
    pub no_data: Option<i16>,
}

impl ResultatViewshed {
    pub fn pixels_visibles(&self) -> usize {
        self.visibilite.iter().filter(|&&pixel| pixel == 1).count()
    }

    pub fn coordonnees_pixel(&self, colonne: usize, ligne: usize) -> Option<(f64, f64)> {
        if colonne >= self.largeur || ligne >= self.hauteur {
            return None;
        }

        let t = self.geotransform;

        let longitude = t[0] + (colonne as f64 + 0.5) * t[1] + (ligne as f64 + 0.5) * t[2];

        let latitude = t[3] + (colonne as f64 + 0.5) * t[4] + (ligne as f64 + 0.5) * t[5];

        Some((longitude, latitude))
    }
}

/// Calcule la distance horizontale entre deux points géographiques.
pub fn distance_metres(point_a: PointObservation, point_b: PointObservation) -> f64 {
    let rayon_terre = 6_371_000.0_f64;

    let lat_a = point_a.latitude.to_radians();
    let lat_b = point_b.latitude.to_radians();
    let delta_lat = (point_b.latitude - point_a.latitude).to_radians();
    let delta_lon = (point_b.longitude - point_a.longitude).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat_a.cos() * lat_b.cos() * (delta_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    rayon_terre * c
}

/// Analyse la visibilité directe entre deux points.
pub fn analyser_los(
    terrain: &Terrain,
    point_a: PointObservation,
    point_b: PointObservation,
) -> Result<ResultatLOS, String> {
    let distance = distance_metres(point_a, point_b);

    if distance == 0.0 {
        return Err("Les deux points sont confondus".to_string());
    }

    let (resolution_x, resolution_y) = terrain.resolution_metres()?;
    let pas_metres = resolution_x.min(resolution_y) / 2.0;
    let nombre_echantillons = (distance / pas_metres).ceil() as usize;
    let altitude_a = terrain.altitude_geo(point_a.longitude, point_a.latitude)?;
    let altitude_b = terrain.altitude_geo(point_b.longitude, point_b.latitude)?;
    let altitude_absolue_a = altitude_a + point_a.hauteur;
    let altitude_absolue_b = altitude_b + point_b.hauteur;
    println!("--- ALTITUDES DES OBSERVATEURS ---");
    println!(
        "Point A : terrain {:.2} m, observateur {:.2} m",
        altitude_a, altitude_absolue_a
    );
    println!(
        "Point B : terrain {:.2} m, observateur {:.2} m",
        altitude_b, altitude_absolue_b
    );
    let difference_altitude = altitude_absolue_b - altitude_absolue_a;

    let angle_vertical = difference_altitude.atan2(distance).to_degrees();
    let rayon_terre = 6_371_000.0_f64;
    for indice in 1..nombre_echantillons {
        let proportion = indice as f64 / nombre_echantillons as f64;

        let (longitude, latitude) = interpoler_position(point_a, point_b, proportion);

        let altitude_terrain = terrain.altitude_geo(longitude, latitude)?;

        let altitude_ligne =
            altitude_absolue_a + (altitude_absolue_b - altitude_absolue_a) * proportion;

        let correction_courbure =
            distance * distance * proportion * (1.0 - proportion) / (2.0 * rayon_terre);

        let altitude_ligne_corrigee = altitude_ligne - correction_courbure;
        if altitude_terrain >= altitude_ligne_corrigee {
            println!("--- OBSTACLE DETECTE ---");
            println!("Position : {:.6}, {:.6}", longitude, latitude);
            println!("Altitude terrain : {:.2} m", altitude_terrain);
            println!("Altitude ligne de visee initiale : {:.2} m", altitude_ligne);
            println!(
                "Altitude ligne de visee corrigee : {:.2} m",
                altitude_ligne_corrigee
            );
            println!(
                "Distance depuis le point A : {:.2} m",
                distance * proportion
            );

            return Ok(ResultatLOS {
                visible: false,
                distance_metres: distance,
                angle_vertical,
            });
        }
    }

    Ok(ResultatLOS {
        visible: true,
        distance_metres: distance,
        angle_vertical,
    })
}

/// Analyse la visibilité sur une route composée de plusieurs points.
pub fn analyser_route(
    terrain: &Terrain,
    points: &[PointObservation],
) -> Result<ResultatRoute, String> {
    if points.len() < 2 {
        return Err("Une route doit contenir au moins deux points".to_string());
    }

    let mut segments = Vec::new();
    let mut distance_totale_metres = 0.0;
    let mut route_entierement_visible = true;

    for paire in points.windows(2) {
        let resultat = analyser_los(terrain, paire[0], paire[1])?;

        distance_totale_metres += resultat.distance_metres;

        if !resultat.visible {
            route_entierement_visible = false;
        }

        segments.push(resultat);
    }

    Ok(ResultatRoute {
        segments,
        distance_totale_metres,
        route_entierement_visible,
    })
}

/// Calcule le Viewshed autour d'un observateur.
/// Calcule le Viewshed autour d'un observateur.
pub fn analyser_viewshed(
    terrain: &Terrain,
    observateur: PointObservation,
    rayon_metres: f64,
) -> Result<ResultatViewshed, String> {
    if rayon_metres <= 0.0 {
        return Err("Le rayon doit être supérieur à zéro".to_string());
    }

    if observateur.hauteur < 0.0 {
        return Err("La hauteur de l'observateur ne peut pas être négative".to_string());
    }

    let (largeur, hauteur) = terrain.dimensions();

    let (colonne_centre, ligne_centre) =
        terrain.coordonnees_pixel(observateur.longitude, observateur.latitude)?;

    let altitudes = terrain.altitudes();
    let no_data = terrain.valeur_no_data();
    let transform = terrain.geotransform()?;

    let indice_centre = ligne_centre * largeur + colonne_centre;
    let altitude_pixel = altitudes[indice_centre];

    if Some(altitude_pixel) == no_data {
        return Err("Le pixel de l'observateur ne contient aucune altitude".to_string());
    }

    let altitude_observateur = altitude_pixel as f64 + observateur.hauteur;

    let mut visibilite = vec![0u8; largeur * hauteur];

    let rayon_terre = 6_371_000.0_f64;

    visibilite
        .par_iter_mut()
        .enumerate()
        .for_each(|(indice, pixel_visible)| {
            let ligne = indice / largeur;
            let colonne = indice % largeur;

            let altitude_cible = altitudes[indice];

            if Some(altitude_cible) == no_data {
                return;
            }

            let longitude = transform[0]
                + (colonne as f64 + 0.5) * transform[1]
                + (ligne as f64 + 0.5) * transform[2];

            let latitude = transform[3]
                + (colonne as f64 + 0.5) * transform[4]
                + (ligne as f64 + 0.5) * transform[5];

            let cible = PointObservation {
                longitude,
                latitude,
                hauteur: 0.0,
            };

            let distance = distance_metres(observateur, cible);

            if distance > rayon_metres {
                return;
            }

            if distance == 0.0 {
                *pixel_visible = 1;
                return;
            }

            let delta_colonne = colonne as f64 - colonne_centre as f64;
            let delta_ligne = ligne as f64 - ligne_centre as f64;

            let nombre_echantillons =
                (delta_colonne.abs().max(delta_ligne.abs()) * 2.0).ceil() as usize;

            let altitude_cible = altitude_cible as f64;
            let mut visible = true;

            for echantillon in 1..nombre_echantillons {
                let proportion = echantillon as f64 / nombre_echantillons as f64;

                let colonne_test =
                    (colonne_centre as f64 + delta_colonne * proportion).round() as usize;

                let ligne_test = (ligne_centre as f64 + delta_ligne * proportion).round() as usize;

                if colonne_test >= largeur || ligne_test >= hauteur {
                    visible = false;
                    break;
                }

                let indice_test = ligne_test * largeur + colonne_test;
                let altitude_test = altitudes[indice_test];

                if Some(altitude_test) == no_data {
                    continue;
                }

                let altitude_ligne =
                    altitude_observateur + (altitude_cible - altitude_observateur) * proportion;

                let correction_courbure =
                    distance * distance * proportion * (1.0 - proportion) / (2.0 * rayon_terre);

                let altitude_ligne_corrigee = altitude_ligne - correction_courbure;

                if altitude_test as f64 >= altitude_ligne_corrigee {
                    visible = false;
                    break;
                }
            }

            if visible {
                *pixel_visible = 1;
            }
        });

    Ok(ResultatViewshed {
        longitude: observateur.longitude,
        latitude: observateur.latitude,
        hauteur_observateur: observateur.hauteur,
        rayon_metres,
        largeur,
        hauteur,
        visibilite,
        geotransform: transform,
        no_data,
    })
}

/// Interpole une position géographique entre deux points.
fn interpoler_position(
    point_a: PointObservation,
    point_b: PointObservation,
    proportion: f64,
) -> (f64, f64) {
    let longitude = point_a.longitude + (point_b.longitude - point_a.longitude) * proportion;

    let latitude = point_a.latitude + (point_b.latitude - point_a.latitude) * proportion;

    (longitude, latitude)
}

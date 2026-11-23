use gdal::Dataset;
use std::path::Path;

/// Module de gestion des données topographiques DTED.
pub struct Terrain {
    dataset: Dataset,
    altitudes: Vec<i16>,
    no_data_value: Option<i16>,
}

impl Terrain {
    /// Ouvre un fichier DTED avec GDAL.
    pub fn ouvrir(chemin: &str) -> Result<Self, String> {
        let dataset = Dataset::open(Path::new(chemin))
            .map_err(|e| format!("Erreur ouverture DTED : {}", e))?;

        let (largeur, hauteur) = dataset.raster_size();

        let bande = dataset
            .rasterband(1)
            .map_err(|e| format!("Erreur accès bande raster : {}", e))?;

        let valeurs = bande
            .read_as::<i16>((0, 0), (largeur, hauteur), (largeur, hauteur), None)
            .map_err(|e| format!("Erreur chargement des altitudes : {}", e))?;

        let altitudes = valeurs.data().to_vec();
        let no_data_value = bande.no_data_value().map(|v| v as i16);

        Ok(Self {
            dataset,
            altitudes,
            no_data_value,
        })
    }

    /// Retourne les dimensions du raster (largeur, hauteur).
    pub fn dimensions(&self) -> (usize, usize) {
        self.dataset.raster_size()
    }

    /// Retourne la résolution approximative du raster en mètres.
    pub fn resolution_metres(&self) -> Result<(f64, f64), String> {
        let transform = self.geotransform()?;

        let latitude = transform[3].to_radians();
        let rayon_terre = 6_371_000.0_f64;

        let resolution_x = transform[1].abs() * latitude.cos() * rayon_terre.to_radians();

        let resolution_y = transform[5].abs() * rayon_terre.to_radians();

        Ok((resolution_x, resolution_y))
    }

    /// Lit l'altitude d'un pixel (colonne, ligne).
    pub fn altitude_pixel(&self, colonne: usize, ligne: usize) -> Result<f64, String> {
        let (largeur, hauteur) = self.dimensions();

        if colonne >= largeur || ligne >= hauteur {
            return Err("Position hors du raster".to_string());
        }

        let indice = ligne * largeur + colonne;
        let altitude = self.altitudes[indice];

        if Some(altitude) == self.no_data_value {
            return Err("Pixel sans donnée d'altitude".to_string());
        }

        Ok(altitude as f64)
    }

    /// Retourne la transformation géographique du raster.
    pub fn geotransform(&self) -> Result<[f64; 6], String> {
        self.dataset
            .geo_transform()
            .map_err(|e| format!("Erreur geotransform : {}", e))
    }
    /// Convertit longitude/latitude en coordonnées pixel.
    /// /// Convertit longitude/latitude en coordonnées pixel.
    pub fn coordonnees_pixel(
        &self,
        longitude: f64,
        latitude: f64,
    ) -> Result<(usize, usize), String> {
        let transform = self.geotransform()?;

        let pixel_x = (longitude - transform[0]) / transform[1];
        let pixel_y = (latitude - transform[3]) / transform[5];

        if pixel_x < 0.0 || pixel_y < 0.0 {
            return Err("Position hors du raster".to_string());
        }

        let colonne = pixel_x.floor() as usize;
        let ligne = pixel_y.floor() as usize;

        let (largeur, hauteur) = self.dimensions();

        if colonne >= largeur || ligne >= hauteur {
            return Err("Position hors du raster".to_string());
        }

        Ok((colonne, ligne))
    }
    /// Convertit des coordonnées pixel en longitude et latitude.
    pub fn coordonnees_geo(&self, colonne: usize, ligne: usize) -> Result<(f64, f64), String> {
        let (largeur, hauteur) = self.dimensions();

        if colonne >= largeur || ligne >= hauteur {
            return Err("Position hors du raster".to_string());
        }

        let transform = self.geotransform()?;

        let longitude = transform[0]
            + (colonne as f64 + 0.5) * transform[1]
            + (ligne as f64 + 0.5) * transform[2];

        let latitude = transform[3]
            + (colonne as f64 + 0.5) * transform[4]
            + (ligne as f64 + 0.5) * transform[5];

        Ok((longitude, latitude))
    }
    /// Retourne l'altitude du terrain à partir de longitude/latitude.
    pub fn altitude_geo(&self, longitude: f64, latitude: f64) -> Result<f64, String> {
        let (colonne, ligne) = self.coordonnees_pixel(longitude, latitude)?;

        self.altitude_pixel(colonne, ligne)
    }

    /// Retourne les limites géographiques du raster.
    pub fn limites(&self) -> Result<(f64, f64, f64, f64), String> {
        let transform = self.geotransform()?;
        let (largeur, hauteur) = self.dimensions();

        let longitude_min = transform[0];
        let latitude_max = transform[3];

        let longitude_max = longitude_min + largeur as f64 * transform[1];

        let latitude_min = latitude_max + hauteur as f64 * transform[5];

        Ok((longitude_min, latitude_min, longitude_max, latitude_max))
    }

    /// Retourne les altitudes chargées en mémoire.
    pub fn altitudes(&self) -> &[i16] {
        &self.altitudes
    }

    /// Retourne la valeur utilisée pour les pixels sans données.
    pub fn valeur_no_data(&self) -> Option<i16> {
        self.no_data_value
    }
}

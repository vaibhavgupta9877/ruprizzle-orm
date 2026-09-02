//! PostGIS and Geospatial Scalar Types for Ruprizzle.
//!
//! Provides geometric data structures ([`Point`], [`Polygon`], [`LineString`],
//! [`MultiPolygon`]) with WKT parsing/formatting, SRID support (defaulting to
//! WGS 84 / `4326`), JSON/GeoJSON serialization, and integration with `Encodable`
//! and query filters.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::value::{Encodable, Value};

/// A 2D geometric coordinate point `(x, y)` / `(lat, lng)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// X coordinate / Latitude.
    pub x: f64,
    /// Y coordinate / Longitude.
    pub y: f64,
    /// Spatial Reference System Identifier (SRID). Default is `4326` (WGS 84).
    #[serde(default = "default_srid")]
    pub srid: u32,
}

fn default_srid() -> u32 {
    4326
}

impl Point {
    /// Creates a new `Point` with default SRID `4326` (WGS 84).
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y, srid: 4326 }
    }

    /// Creates a new `Point` with an explicit SRID.
    #[must_use]
    pub const fn with_srid(x: f64, y: f64, srid: u32) -> Self {
        Self { x, y, srid }
    }

    /// Latitude accessor (alias for `x`).
    #[must_use]
    pub const fn lat(&self) -> f64 {
        self.x
    }

    /// Longitude accessor (alias for `y`).
    #[must_use]
    pub const fn lng(&self) -> f64 {
        self.y
    }

    /// Computes the Euclidean distance to another point.
    #[must_use]
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns the Well-Known Text (WKT) representation.
    #[must_use]
    pub fn to_wkt(&self) -> String {
        format!("POINT({} {})", self.x, self.y)
    }

    /// Returns the EWKT representation including the SRID.
    #[must_use]
    pub fn to_ewkt(&self) -> String {
        format!("SRID={};POINT({} {})", self.srid, self.x, self.y)
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "POINT({} {})", self.x, self.y)
    }
}

impl FromStr for Point {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let mut srid = 4326;
        let body = if s.starts_with("SRID=") {
            let semi = s
                .find(';')
                .ok_or_else(|| format!("malformed EWKT missing ';': {s}"))?;
            let srid_str = s.get(5..semi).unwrap_or("");
            srid = srid_str
                .parse::<u32>()
                .map_err(|e| format!("invalid SRID {srid_str}: {e}"))?;
            s.get(semi + 1..).unwrap_or("")
        } else {
            s
        };

        let body = body.trim();
        if let Some(coords_str) = body
            .strip_prefix("POINT")
            .or_else(|| body.strip_prefix("point"))
        {
            let trimmed = coords_str.trim();
            if trimmed.starts_with('(') && trimmed.ends_with(')') {
                let inner = trimmed
                    .get(1..trimmed.len().saturating_sub(1))
                    .unwrap_or("")
                    .trim();
                let parts: Vec<&str> = inner.split_whitespace().collect();
                if let (Some(x_part), Some(y_part)) = (parts.first(), parts.get(1)) {
                    let x = x_part
                        .parse::<f64>()
                        .map_err(|e| format!("invalid x coordinate: {e}"))?;
                    let y = y_part
                        .parse::<f64>()
                        .map_err(|e| format!("invalid y coordinate: {e}"))?;
                    return Ok(Point::with_srid(x, y, srid));
                }
            }
        }

        // Try comma-separated "x, y"
        if let Some((x_str, y_str)) = s.split_once(',') {
            let x = x_str
                .trim()
                .parse::<f64>()
                .map_err(|e| format!("invalid x coordinate: {e}"))?;
            let y = y_str
                .trim()
                .parse::<f64>()
                .map_err(|e| format!("invalid y coordinate: {e}"))?;
            return Ok(Point::with_srid(x, y, srid));
        }

        Err(format!("cannot parse Point from '{s}'"))
    }
}

impl Encodable for Point {
    fn to_value(&self) -> Value {
        Value::Str(self.to_wkt().into())
    }
}

/// A 2D geometric line string consisting of consecutive line segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineString {
    /// Points forming the linestring in order.
    pub points: Vec<Point>,
    /// SRID identifier.
    #[serde(default = "default_srid")]
    pub srid: u32,
}

impl LineString {
    /// Creates a new `LineString` with default SRID `4326`.
    #[must_use]
    pub fn new(points: Vec<Point>) -> Self {
        Self { points, srid: 4326 }
    }

    /// Returns the WKT representation.
    #[must_use]
    pub fn to_wkt(&self) -> String {
        let pts = self
            .points
            .iter()
            .map(|p| format!("{} {}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(", ");
        format!("LINESTRING({pts})")
    }
}

impl fmt::Display for LineString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wkt())
    }
}

impl FromStr for LineString {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let body = if let Some(stripped) = s.strip_prefix("LINESTRING") {
            stripped.trim()
        } else if let Some(stripped) = s.strip_prefix("linestring") {
            stripped.trim()
        } else {
            s
        };

        if body.starts_with('(') && body.ends_with(')') {
            let inner = body
                .get(1..body.len().saturating_sub(1))
                .unwrap_or("")
                .trim();
            let mut points = Vec::new();
            for part in inner.split(',') {
                let coords: Vec<&str> = part.split_whitespace().collect();
                if let (Some(x_coord), Some(y_coord)) = (coords.first(), coords.get(1)) {
                    let x = x_coord
                        .parse::<f64>()
                        .map_err(|e| format!("invalid x: {e}"))?;
                    let y = y_coord
                        .parse::<f64>()
                        .map_err(|e| format!("invalid y: {e}"))?;
                    points.push(Point::new(x, y));
                }
            }
            return Ok(LineString::new(points));
        }

        Err(format!("cannot parse LineString from '{s}'"))
    }
}

impl Encodable for LineString {
    fn to_value(&self) -> Value {
        Value::Str(self.to_wkt().into())
    }
}

/// A 2D geometric polygon consisting of an exterior ring and optional interior rings (holes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    /// Rings of the polygon: `rings[0]` is the exterior ring; `rings[1..]` are holes.
    pub rings: Vec<Vec<Point>>,
    /// SRID identifier.
    #[serde(default = "default_srid")]
    pub srid: u32,
}

impl Polygon {
    /// Creates a polygon from an exterior ring of points.
    #[must_use]
    pub fn new(exterior: Vec<Point>) -> Self {
        Self {
            rings: vec![exterior],
            srid: 4326,
        }
    }

    /// Creates a polygon with an exterior ring and one or more interior holes.
    #[must_use]
    pub fn with_holes(exterior: Vec<Point>, holes: Vec<Vec<Point>>) -> Self {
        let mut rings = Vec::with_capacity(1 + holes.len());
        rings.push(exterior);
        rings.extend(holes);
        Self { rings, srid: 4326 }
    }

    /// Returns the WKT representation.
    #[must_use]
    pub fn to_wkt(&self) -> String {
        let rings_wkt = self
            .rings
            .iter()
            .map(|ring| {
                let pts = ring
                    .iter()
                    .map(|p| format!("{} {}", p.x, p.y))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({pts})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("POLYGON({rings_wkt})")
    }
}

impl fmt::Display for Polygon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wkt())
    }
}

impl FromStr for Polygon {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let body = if let Some(stripped) = s.strip_prefix("POLYGON") {
            stripped.trim()
        } else if let Some(stripped) = s.strip_prefix("polygon") {
            stripped.trim()
        } else {
            s
        };

        if body.starts_with('(') && body.ends_with(')') {
            let inner = body
                .get(1..body.len().saturating_sub(1))
                .unwrap_or("")
                .trim();
            let mut rings = Vec::new();
            for ring_str in inner.split("),") {
                let ring_str = ring_str
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')');
                let mut ring = Vec::new();
                for pt_str in ring_str.split(',') {
                    let coords: Vec<&str> = pt_str.split_whitespace().collect();
                    if let (Some(x_coord), Some(y_coord)) = (coords.first(), coords.get(1)) {
                        let x = x_coord
                            .parse::<f64>()
                            .map_err(|e| format!("invalid x: {e}"))?;
                        let y = y_coord
                            .parse::<f64>()
                            .map_err(|e| format!("invalid y: {e}"))?;
                        ring.push(Point::new(x, y));
                    }
                }
                if !ring.is_empty() {
                    rings.push(ring);
                }
            }
            return Ok(Polygon { rings, srid: 4326 });
        }

        Err(format!("cannot parse Polygon from '{s}'"))
    }
}

impl Encodable for Polygon {
    fn to_value(&self) -> Value {
        Value::Str(self.to_wkt().into())
    }
}

/// A 2D geometric collection of polygons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiPolygon {
    /// Individual polygons in the collection.
    pub polygons: Vec<Polygon>,
    /// SRID identifier.
    #[serde(default = "default_srid")]
    pub srid: u32,
}

impl MultiPolygon {
    /// Creates a new `MultiPolygon`.
    #[must_use]
    pub fn new(polygons: Vec<Polygon>) -> Self {
        Self {
            polygons,
            srid: 4326,
        }
    }

    /// Returns the WKT representation.
    #[must_use]
    pub fn to_wkt(&self) -> String {
        let polys_wkt = self
            .polygons
            .iter()
            .map(|poly| {
                let rings_wkt = poly
                    .rings
                    .iter()
                    .map(|ring| {
                        let pts = ring
                            .iter()
                            .map(|p| format!("{} {}", p.x, p.y))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("({pts})")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({rings_wkt})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("MULTIPOLYGON({polys_wkt})")
    }
}

impl fmt::Display for MultiPolygon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wkt())
    }
}

impl FromStr for MultiPolygon {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let body = if let Some(stripped) = s.strip_prefix("MULTIPOLYGON") {
            stripped.trim()
        } else if let Some(stripped) = s.strip_prefix("multipolygon") {
            stripped.trim()
        } else {
            s
        };

        if body.starts_with('(') && body.ends_with(')') {
            let inner = body
                .get(1..body.len().saturating_sub(1))
                .unwrap_or("")
                .trim();
            let mut polygons = Vec::new();
            for poly_str in inner.split(")),") {
                let poly_str = format!("{poly_str}))");
                if let Ok(poly) = poly_str.trim().parse::<Polygon>() {
                    polygons.push(poly);
                }
            }
            return Ok(MultiPolygon::new(polygons));
        }

        Err(format!("cannot parse MultiPolygon from '{s}'"))
    }
}

impl Encodable for MultiPolygon {
    fn to_value(&self) -> Value {
        Value::Str(self.to_wkt().into())
    }
}

//! PostGIS Geospatial Tests for Ruprizzle.

use ruprizzle::prelude::*;
use ruprizzle::spatial::{LineString, MultiPolygon, Point, Polygon};
use ruprizzle::sqlx;
use ruprizzle::{FilterNode, SpatialOp};

#[test]
fn point_wkt_and_distance() {
    let p1 = Point::new(0.0, 0.0);
    let p2 = Point::new(3.0, 4.0);

    assert_eq!(p1.to_wkt(), "POINT(0 0)");
    assert_eq!(p2.to_wkt(), "POINT(3 4)");
    assert_eq!(p1.to_ewkt(), "SRID=4326;POINT(0 0)");
    assert_eq!(p1.distance_to(&p2), 5.0);

    // Parsing
    let parsed: Point = "POINT(10.5 20.75)".parse().unwrap();
    assert_eq!(parsed.x, 10.5);
    assert_eq!(parsed.y, 20.75);

    let parsed_ewkt: Point = "SRID=3857;POINT(100 200)".parse().unwrap();
    assert_eq!(parsed_ewkt.x, 100.0);
    assert_eq!(parsed_ewkt.y, 200.0);
    assert_eq!(parsed_ewkt.srid, 3857);
}

#[test]
fn linestring_and_polygon_wkt() {
    let ls = LineString::new(vec![
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(2.0, 2.0),
    ]);
    assert_eq!(ls.to_wkt(), "LINESTRING(0 0, 1 1, 2 2)");

    let poly = Polygon::new(vec![
        Point::new(0.0, 0.0),
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
        Point::new(10.0, 0.0),
        Point::new(0.0, 0.0),
    ]);
    assert_eq!(poly.to_wkt(), "POLYGON((0 0, 0 10, 10 10, 10 0, 0 0))");

    let parsed_poly: Polygon = poly.to_wkt().parse().unwrap();
    assert_eq!(parsed_poly.rings.len(), 1);
    assert_eq!(parsed_poly.rings[0].len(), 5);

    let multi = MultiPolygon::new(vec![poly]);
    assert!(multi.to_wkt().starts_with("MULTIPOLYGON("));
}

#[derive(Default)]
struct Location;

impl<'r> sqlx::FromRow<'r, sqlx::any::AnyRow> for Location {
    fn from_row(_: &'r sqlx::any::AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Location)
    }
}
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for Location {
    fn from_row(_: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Location)
    }
}
impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for Location {
    fn from_row(_: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Location)
    }
}
impl<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> for Location {
    fn from_row(_: &'r sqlx::mysql::MySqlRow) -> Result<Self, sqlx::Error> {
        Ok(Location)
    }
}

#[cfg(feature = "postgres-tokio-postgres")]
ruprizzle::tokio_postgres_default_row!(Location);

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromRusqliteRow for Location {
    fn from_rusqlite_row(_: &ruprizzle::rusqlite::RusqliteRow) -> Result<Self, ruprizzle::Error> {
        Ok(Location)
    }
}

#[cfg(feature = "sqlite-rusqlite")]
impl ruprizzle::rusqlite::FromOwnedRow for Location {
    fn from_owned_row(_: &ruprizzle::rusqlite::Row) -> Result<Self, ruprizzle::Error> {
        Ok(Location)
    }
}

impl Model for Location {
    const TABLE: &'static str = "locations";
    const PRIMARY_KEY: &'static str = "id";
    const COLUMNS: &'static [&'static str] = &["id", "name", "coords", "boundary"];
}

const COORDS: Column<Location, Point> = Column::new("locations", "coords");
const BOUNDARY: Column<Location, Polygon> = Column::new("locations", "boundary");

#[test]
fn spatial_query_filters() {
    let origin = Point::new(10.0, 20.0);
    let f1 = COORDS.within_radius(&origin, 1000.0);
    assert!(matches!(
        f1.node,
        FilterNode::Spatial {
            op: SpatialOp::WithinRadius,
            ..
        }
    ));

    let order = COORDS.distance_asc(&origin);
    assert!(order.spatial_distance.is_some());

    let poly = Polygon::new(vec![
        Point::new(0.0, 0.0),
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 0.0),
    ]);
    let f2 = BOUNDARY.contains(&poly);
    assert!(matches!(
        f2.node,
        FilterNode::Spatial {
            op: SpatialOp::Contains,
            ..
        }
    ));
}

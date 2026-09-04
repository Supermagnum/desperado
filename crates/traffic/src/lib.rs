//! Traffic-event decoding shared by `dabradio` (TPEG/TEC) and `fmradio` (RDS-TMC).
//!
//! DAB packet-mode and FIG parsing stay in `dabradio`; this crate is the
//! application layer (transport frames, TEC, ALERT-C) plus the common GeoJSON
//! output shape.

pub mod crc;
pub mod geojson;
pub mod location;
pub mod tmc;
pub mod tpeg;

pub use geojson::{Bearer, FeatureCollection, Geometry, TrafficFeature, TrafficProperties};
pub use location::LocationTable;
pub use tmc::{TmcDecoder, is_tmc_aid};
pub use tpeg::{decode_payload, find_frames};

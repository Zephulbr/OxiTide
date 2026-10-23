//! Persisted ids in the V2 ref form (`tidal:<n>`, V2 spec V2-D19 / V2-D34).
//! 1.x keeps `u64` ids in memory and on the IPC wire; only the files it
//! writes (`session.json`, `history.jsonl`) carry the string form, so 2.0 can
//! read them without a rewrite pass. Reads accept either form: older files
//! hold bare numbers.
//!
//! Use through `#[serde(with = "…")]`: [`id`] for a `u64`, [`opt_id`] for an
//! `Option<u64>`, [`artist_refs`] for name-link artists, written as V2's
//! `ArtistLink` shape `{ "ref": "tidal:<n>", "name": … }`.

use oxitide_audio::queue::ArtistRef;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The only source 1.x has.
const TIDAL_PREFIX: &str = "tidal:";

/// `tidal:<id>`.
fn to_ref(id: u64) -> String {
    format!("{TIDAL_PREFIX}{id}")
}

/// An id as found on disk: a bare number (files written before 1.0) or a ref.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawId {
    Number(u64),
    Ref(String),
}

impl RawId {
    fn into_id<E: serde::de::Error>(self) -> Result<u64, E> {
        match self {
            RawId::Number(id) => Ok(id),
            RawId::Ref(text) => text
                .strip_prefix(TIDAL_PREFIX)
                .and_then(|id| id.parse().ok())
                .ok_or_else(|| E::custom(format!("not a TIDAL id: {text:?}"))),
        }
    }
}

pub mod id {
    use super::*;

    pub fn serialize<S: Serializer>(id: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&to_ref(*id))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        RawId::deserialize(deserializer)?.into_id()
    }
}

pub mod opt_id {
    use super::*;

    pub fn serialize<S: Serializer>(id: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
        id.map(to_ref).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
        Option::<RawId>::deserialize(deserializer)?.map(RawId::into_id).transpose()
    }
}

pub mod artist_refs {
    use super::*;

    #[derive(Serialize)]
    struct LinkOut<'a> {
        #[serde(rename = "ref")]
        reference: String,
        name: &'a str,
    }

    /// V2's `{ ref, name }`, or the 1.x `{ id, name }`.
    #[derive(Deserialize)]
    struct LinkIn {
        #[serde(rename = "ref", default)]
        reference: Option<RawId>,
        #[serde(default)]
        id: Option<RawId>,
        #[serde(default)]
        name: String,
    }

    pub fn serialize<S: Serializer>(refs: &[ArtistRef], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(refs.iter().map(|a| LinkOut { reference: to_ref(a.id), name: &a.name }))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<ArtistRef>, D::Error> {
        Vec::<LinkIn>::deserialize(deserializer)?
            .into_iter()
            .map(|link| {
                let raw = link
                    .reference
                    .or(link.id)
                    .ok_or_else(|| D::Error::custom("artist link without an id"))?;
                Ok(ArtistRef { id: raw.into_id()?, name: link.name })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        #[serde(with = "id")]
        track_id: u64,
        #[serde(default, with = "opt_id", skip_serializing_if = "Option::is_none")]
        album_id: Option<u64>,
        #[serde(default, with = "artist_refs", skip_serializing_if = "Vec::is_empty")]
        artist_refs: Vec<ArtistRef>,
    }

    fn sample() -> Sample {
        Sample {
            track_id: 12,
            album_id: Some(34),
            artist_refs: vec![ArtistRef { id: 56, name: "A".into() }],
        }
    }

    #[test]
    fn writes_the_v2_ref_form() {
        assert_eq!(
            serde_json::to_value(sample()).unwrap(),
            json!({
                "track_id": "tidal:12",
                "album_id": "tidal:34",
                "artist_refs": [{ "ref": "tidal:56", "name": "A" }],
            })
        );
    }

    #[test]
    fn reads_what_it_writes() {
        let text = serde_json::to_string(&sample()).unwrap();
        assert_eq!(serde_json::from_str::<Sample>(&text).unwrap(), sample());
    }

    #[test]
    fn reads_the_bare_number_shapes_of_older_files() {
        let legacy = json!({
            "track_id": 12,
            "album_id": 34,
            "artist_refs": [{ "id": 56, "name": "A" }],
        });
        assert_eq!(serde_json::from_value::<Sample>(legacy).unwrap(), sample());
        let without = json!({ "track_id": 12, "album_id": null });
        let read = serde_json::from_value::<Sample>(without).unwrap();
        assert_eq!((read.album_id, read.artist_refs.len()), (None, 0));
    }

    #[test]
    fn rejects_ids_1x_cannot_play() {
        for bad in [json!({ "track_id": "local:abc" }), json!({ "track_id": "tidal:x" })] {
            assert!(serde_json::from_value::<Sample>(bad).is_err());
        }
        let no_id = json!({ "track_id": 1, "artist_refs": [{ "name": "A" }] });
        assert!(serde_json::from_value::<Sample>(no_id).is_err());
    }
}

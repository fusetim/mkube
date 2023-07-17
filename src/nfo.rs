use serde::{Serialize, Deserialize};

#[serde(rename = "movie")]
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Movie {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub original_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub plot: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub uniqueid: Vec<UniqueId>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub genre: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub tag: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub country: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub credits: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub director: Vec<CrewPerson>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub producer: Vec<CrewPerson>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub premiered: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub studio: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub actor: Vec<Actor>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub thumb: Vec<Thumb>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub runtime: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub tagline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub fileinfo: Option<FileInfo>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub source: Option<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct UniqueId {
    #[serde(rename = "@type")]
    pub id_type: String,
    #[serde(rename = "@default", default)]
    pub default: bool,
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Actor {
    pub name: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub role: Vec<String>,
    pub order: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub tmdbid: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub thumb: Option<Thumb>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct CrewPerson {
    pub name: String,
    #[serde(rename = "@tmdbid")]
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub tmdbid: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub thumb: Option<Thumb>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Thumb {
    #[serde(rename = "@aspect")]
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub aspect: Option<String>,
    #[serde(rename = "$value")]
    pub path: String,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct FileInfo { 
    pub streamdetails: StreamDetails,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct StreamDetails { 
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub video: Vec<VideoTrack>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub audio: Vec<AudioTrack>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")] 
    pub subtitle: Vec<SubtitleTrack>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct VideoTrack { 
    pub codec: String,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub aspect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub width: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub height: Option<u64>,
    #[serde(rename = "durationinseconds")]
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub duration_in_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub language: Option<String>,
    #[serde(rename = "hdrtype")]
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub hdr_type: Option<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct AudioTrack { 
    pub codec: String,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub channels: Option<u64>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct SubtitleTrack { 
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub language: Option<String>,
}

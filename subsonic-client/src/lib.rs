mod auth;

pub use auth::SubsonicAuth;

use bytes::Bytes;
use reqwest::StatusCode;
use serde::{de::DeserializeOwned, Deserialize};
use thiserror::Error;
use tracing::info;
use url::Url;

pub struct Client {
    auth: SubsonicAuth,
    base_url: Url,
    client: reqwest::Client,
}

impl Client {
    pub fn new(auth: SubsonicAuth, base_url: &str) -> Self {
        // TODO err
        let base_url = url::Url::parse(base_url).unwrap();

        Self {
            auth,
            base_url,
            client: reqwest::ClientBuilder::new()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        }
    }

    fn build_req_url(&self, endpoint: &str, args: &[(&str, String)]) -> Result<Url, ApiError> {
        let mut req_url = self.base_url.clone();
        req_url
            .path_segments_mut()
            .map_err(|_| ApiError::UrlCannotBeABaseUrl)?
            .push(endpoint);

        {
            let mut query_pairs = req_url.query_pairs_mut();
            self.auth.add_to_query_pairs(&mut query_pairs);

            for (k, v) in args.iter() {
                query_pairs.append_pair(k, v);
            }
        }

        Ok(req_url)
    }

    async fn request_raw<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        args: &[(&str, String)],
    ) -> ApiResult<ResponseEnvelope<T>> {
        let req_url = self.build_req_url(endpoint, args)?;

        info!(url = req_url.to_string().as_str(), "Subsonic API Request");
        let res = self.client.get(req_url).send().await?.error_for_status()?;
        let status = res.status();

        let res = match res.json::<ResponseEnvelope<T>>().await {
            Ok(json) => json,
            Err(e) => {
                if status == StatusCode::FOUND {
                    return Err(ApiError::SuspiciousRedirect);
                } else {
                    return Err(e.into());
                }
            }
        };

        if let Some(e) = res.body.error {
            let message = e.message.unwrap_or_else(|| "".to_string());
            let err = match e.code {
                0 => ApiError::Generic(message),
                10 => ApiError::RequiredParameterMissing(message),
                20 => ApiError::ClientMustUpgrade(message),
                30 => ApiError::ServerMustUpgrade(message),
                40 => ApiError::WrongUsernameOrPassword(message),
                41 => ApiError::NoTokenAuthForLdap(message),
                50 => ApiError::UserNotAuthorized(message),
                60 => ApiError::TrialExpired(message),
                70 => ApiError::NotFound(message),
                _ => ApiError::Generic(message),
            };
            Err(err)
        } else if res.body.payload.is_none() {
            Err(ApiError::MalformedApiResponse)
        } else {
            Ok(res)
        }
    }

    async fn request<T: DeserializeOwned>(&self, endpoint: &str) -> ApiResult<T> {
        // SAFETY: request_raw will error out if payload is None
        Ok(self.request_raw(endpoint, &[]).await?.body.payload.unwrap())
    }

    async fn request_args<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        args: &[(&str, String)],
    ) -> ApiResult<T> {
        // SAFETY: request_raw will error out if payload is None
        Ok(self
            .request_raw(endpoint, args)
            .await?
            .body
            .payload
            .unwrap())
    }

    pub async fn ping(&self) -> ApiResult<RequestInfo> {
        let res = self.request_raw::<PingBody>("ping", &[]).await?;
        Ok(res.body.info)
    }

    pub async fn get_license(&self) -> ApiResult<License> {
        let res = self.request::<GetLicenseBody>("getLicense").await?;
        Ok(res.license)
    }

    pub async fn get_music_folders(&self) -> ApiResult<Vec<MusicFolder>> {
        let res = self
            .request::<GetMusicFoldersBody>("getMusicFolders")
            .await?;
        Ok(res.inner.folder)
    }

    pub async fn get_genres(&self) -> ApiResult<Vec<Genre>> {
        let res = self.request::<GetGenresBody>("getGenres").await?;
        Ok(res.inner.genre)
    }

    pub async fn get_indexes(&self) -> ApiResult<IndexesInfo> {
        let res = self.request::<GetIndexesBody>("getIndexes").await?;
        Ok(IndexesInfo {
            last_modified: res.inner.last_modified,
            ignored_articles: res.inner.ignored_articles,
            indexes: res.inner.indexes.unwrap_or_default(),
            shortcuts: res.inner.shortcuts.unwrap_or_default(),
            children: res.inner.children.unwrap_or_default(),
        })
    }

    pub async fn get_music_directory(&self, id: &str) -> ApiResult<Directory> {
        let res = self
            .request_args::<GetMusicDirectoryBody>("getMusicDirectory", &[("id", id.to_owned())])
            .await?;
        Ok(Directory {
            id: res.inner.id,
            parent: res.inner.parent,
            name: res.inner.name,
            starred: res.inner.starred,
            user_rating: res.inner.user_rating,
            average_rating: res.inner.average_rating,
            play_count: res.inner.play_count,
            children: res.inner.children.unwrap_or_default(),
        })
    }

    pub async fn get_artists(
        &self,
        music_folder_id: impl Into<Option<&str>>,
    ) -> ApiResult<IndexesInfo> {
        let mut params = vec![];
        if let Some(id) = music_folder_id.into() {
            params.push(("musicFolderId", id.to_owned()));
        }

        let res = self
            .request_args::<GetArtistsBody>("getArtists", &params)
            .await?;

        Ok(IndexesInfo {
            last_modified: res.inner.last_modified,
            ignored_articles: res.inner.ignored_articles,
            indexes: res.inner.indexes.unwrap_or_default(),
            shortcuts: res.inner.shortcuts.unwrap_or_default(),
            children: res.inner.children.unwrap_or_default(),
        })
    }

    pub async fn get_artist(&self, id: &str) -> ApiResult<ArtistWithAlbumsId3> {
        let res = self
            .request_args::<GetArtistBody>("getArtist", &[("id", id.to_owned())])
            .await?;

        Ok(ArtistWithAlbumsId3 {
            artist_id3: res.inner.artist_id3,
            albums: res.inner.albums.unwrap_or_default(),
        })
    }

    pub async fn get_album(&self, id: &str) -> ApiResult<AlbumWithSongsId3> {
        let res = self
            .request_args::<GetAlbumBody>("getAlbum", &[("id", id.to_owned())])
            .await?;

        Ok(AlbumWithSongsId3 {
            album_id3: res.inner.album_id3,
            songs: res.inner.songs.unwrap_or_default(),
        })
    }

    pub async fn get_artist_info(
        &self,
        id: &str,
        count: Option<usize>,
        include_not_present: Option<bool>,
    ) -> ApiResult<ArtistInfo> {
        let mut params = vec![("id", id.to_owned())];
        if let Some(count) = count {
            params.push(("count", count.to_string()));
        }
        if let Some(include_not_present) = include_not_present {
            params.push(("include_not_present", include_not_present.to_string()));
        }

        let res = self
            .request_args::<GetArtistInfoBody>("getArtistInfo", &[])
            .await?;

        Ok(ArtistInfo {
            base: res.inner.base,
            similar_artists: res.inner.similar_artists.unwrap_or_default(),
        })
    }

    pub async fn get_artist_info_2(&self, id: &str) -> ApiResult<ArtistInfo2> {
        let res = self
            .request_args::<GetArtistInfo2Body>("getArtistInfo2", &[("id", id.to_owned())])
            .await?;

        Ok(ArtistInfo2 {
            base: res.inner.base,
            similar_artists: res.inner.similar_artists.unwrap_or_default(),
        })
    }

    pub async fn get_album_info(&self, id: &str) -> ApiResult<AlbumInfo> {
        let res = self
            .request_args::<GetAlbumInfoBody>("getAlbumInfo", &[("id", id.to_owned())])
            .await?;
        Ok(res.album_info)
    }

    pub async fn get_album_info_2(&self, id: &str) -> ApiResult<AlbumInfo> {
        let res = self
            .request_args::<GetAlbumInfoBody>("getAlbumInfo", &[("id", id.to_owned())])
            .await?;
        Ok(res.album_info)
    }

    pub async fn get_album_list(
        &self,
        list_type: AlbumListType,
        size: Option<usize>,
        offset: Option<usize>,
        music_folder_id: Option<&str>,
    ) -> ApiResult<Vec<Child>> {
        let mut params = vec![];
        params.push(("type", list_type.to_param_str().to_owned()));
        match list_type {
            AlbumListType::ByYear { from_year, to_year } => {
                params.push(("fromYear", from_year.to_string()));
                params.push(("toYear", to_year.to_string()));
            }
            AlbumListType::ByGenre { genre } => {
                params.push(("genre", genre.clone()));
            }
            _ => (),
        }
        if let Some(size) = size {
            params.push(("size", size.to_string()));
        }
        if let Some(offset) = offset {
            params.push(("offset", offset.to_string()));
        }
        if let Some(music_folder_id) = music_folder_id {
            params.push(("music_folder_id", music_folder_id.to_owned()));
        }

        let res = self
            .request_args::<GetAlbumListBody>("getAlbumList", &params)
            .await?;
        Ok(res.inner.album.unwrap_or_default())
    }

    pub fn cover_art_url(&self, id: &str, size: Option<usize>) -> Result<Url, ApiError> {
        let mut params = vec![("id", id.to_owned())];
        if let Some(size) = size {
            params.push(("size", size.to_string()));
        }

        self.build_req_url("getCoverArt", &params)
    }

    pub async fn get_cover_art(&self, id: &str, size: Option<usize>) -> Result<Bytes, ApiError> {
        let url = self.cover_art_url(id, size)?;
        info!(url = url.to_string().as_str(), "Subsonic API Request");
        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?)
    }

    pub fn stream_url(
        &self,
        id: &str,
        max_bit_rate: Option<usize>,
        format: Option<&str>,
        time_offset: Option<usize>,
        estimate_content_length: Option<bool>,
        converted: Option<bool>,
    ) -> Result<Url, ApiError> {
        let mut params = vec![("id", id.to_owned())];
        if let Some(max_bit_rate) = max_bit_rate {
            params.push(("max_bit_rate", max_bit_rate.to_string()));
        }
        if let Some(format) = format {
            params.push(("format", format.to_owned()));
        }
        if let Some(time_offset) = time_offset {
            params.push(("time_offset", time_offset.to_string()));
        }
        if let Some(estimate_content_length) = estimate_content_length {
            params.push((
                "estimate_content_length",
                estimate_content_length.to_string(),
            ));
        }
        if let Some(converted) = converted {
            params.push(("converted", converted.to_string()));
        }

        self.build_req_url("stream", &params)
    }

    pub async fn stream(
        &self,
        id: &str,
        max_bit_rate: Option<usize>,
        format: Option<&str>,
        time_offset: Option<usize>,
        estimate_content_length: Option<bool>,
        converted: Option<bool>,
    ) -> Result<Bytes, ApiError> {
        let url = self.stream_url(
            id,
            max_bit_rate,
            format,
            time_offset,
            estimate_content_length,
            converted,
        )?;

        info!(url = url.to_string().as_str(), "Subsonic API Request");
        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?)
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("The configured url is not a valid base url")]
    UrlCannotBeABaseUrl,

    #[error("Request error")]
    RequestError(#[from] reqwest::Error),

    #[error("{0}")]
    Generic(String),

    #[error("Required Parameter is missing. ({0})")]
    RequiredParameterMissing(String),

    #[error("Incompatible Subsonic REST protocol version. Client must upgrade. ({0})")]
    ClientMustUpgrade(String),

    #[error("Incompatible Subsonic REST protocol version. Server must upgrade. ({0})")]
    ServerMustUpgrade(String),

    #[error("Wrong username or password. ({0})")]
    WrongUsernameOrPassword(String),

    #[error("Token authentication not supported for LDAP users. ({0})")]
    NoTokenAuthForLdap(String),

    #[error("User is not authorized for the given operation. ({0})")]
    UserNotAuthorized(String),

    #[error("The trial period for the Subsonic server is over. Please upgrade to Subsonic Premium. Visit subsonic.org for details. ({0})")]
    TrialExpired(String),

    #[error("The requested data was not found. ({0})")]
    NotFound(String),

    #[error("Malformed API Response")]
    MalformedApiResponse,

    #[error("The server returned a redirect, but the response could not be parsed. Check your base url; Navidrome does this if you have a bogus path there.")]
    SuspiciousRedirect,
}

#[derive(Debug, Deserialize)]
pub struct ResponseEnvelope<T> {
    #[serde(rename = "subsonic-response")]
    body: ResponseBody<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBody<T> {
    #[serde(flatten)]
    info: RequestInfo,

    #[serde(flatten)]
    payload: Option<T>,

    error: Option<ResponseError>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseError {
    code: i32,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestInfo {
    pub status: ResponseStatus,
    pub version: String,
    #[serde(rename = "type")]
    pub server_type: String,
    pub server_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResponseStatus {
    Ok,
    Failed,
}

//////////
// ping //
//////////

#[derive(Debug, Deserialize)]
pub struct PingBody {}

////////////////
// getLicense //
////////////////

#[derive(Debug, Deserialize)]
pub struct GetLicenseBody {
    license: License,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub valid: bool,
    pub email: Option<String>,
    pub license_expires: Option<String>,
    pub trial_expires: Option<String>,
}

/////////////////////
// getMusicFolders //
/////////////////////

#[derive(Debug, Deserialize)]
struct GetMusicFoldersBody {
    #[serde(rename = "musicFolders")]
    inner: GetMusicFoldersInner,
}

#[derive(Debug, Deserialize)]
struct GetMusicFoldersInner {
    #[serde(rename = "musicFolder")]
    folder: Vec<MusicFolder>,
}

#[derive(Debug, Deserialize)]
pub struct MusicFolder {
    pub id: i64,
    pub name: String,
}

////////////////
// getIndexes //
////////////////

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetIndexesBody {
    #[serde(rename = "indexes")]
    inner: GetIndexesInner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetIndexesInner {
    last_modified: i64,
    ignored_articles: String,

    #[serde(rename = "index")]
    indexes: Option<Vec<Index>>,

    #[serde(rename = "shortcut")]
    shortcuts: Option<Vec<Artist>>,

    #[serde(rename = "child")]
    children: Option<Vec<Child>>,
}

#[derive(Debug)]
pub struct IndexesInfo {
    pub last_modified: i64,
    pub ignored_articles: String,
    pub indexes: Vec<Index>,
    pub shortcuts: Vec<Artist>,
    pub children: Vec<Child>,
}

#[derive(Debug, Deserialize)]
pub struct Index {
    pub name: String,
    pub artist: Vec<Artist>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub artist_image_url: Option<String>,
    pub starred: Option<String>,
    pub user_rating: Option<u8>,
    pub average_rating: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Child {
    pub id: String,
    pub parent: Option<String>,
    pub is_dir: bool,
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub track: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub size: Option<i64>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    pub transcoded_content_type: Option<String>,
    pub transcoded_soffix: Option<String>,
    pub duration: Option<i32>,
    pub bit_rate: Option<i32>,
    pub is_video: Option<bool>,
    pub user_rating: Option<u8>,
    pub average_rating: Option<f32>,
    pub play_count: Option<i64>,
    pub disc_number: Option<i32>,
    pub created: Option<String>,
    pub starred: Option<String>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    #[serde(rename = "type")]
    pub media_type: Option<MediaType>,
    pub bookmark_position: Option<i64>,
    pub original_width: Option<i32>,
    pub original_height: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaType {
    Music,
    Podcast,
    Audiobook,
    Video,
}

///////////////////////
// getMusicDirectory //
///////////////////////

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetMusicDirectoryBody {
    #[serde(rename = "directory")]
    inner: GetMusicDirectoryInner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMusicDirectoryInner {
    id: String,
    parent: Option<String>,
    name: String,
    starred: Option<String>,
    user_rating: Option<u8>,
    average_rating: Option<f32>,
    play_count: Option<i64>,
    #[serde(rename = "child")]
    children: Option<Vec<Child>>,
}

#[derive(Debug)]
pub struct Directory {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub starred: Option<String>,
    pub user_rating: Option<u8>,
    pub average_rating: Option<f32>,
    pub play_count: Option<i64>,
    pub children: Vec<Child>,
}

///////////////
// getGenres //
///////////////

#[derive(Debug, Deserialize)]
struct GetGenresBody {
    #[serde(rename = "genres")]
    inner: GetGenresInner,
}

#[derive(Debug, Deserialize)]
struct GetGenresInner {
    genre: Vec<Genre>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub song_count: i64,

    pub album_count: i64,

    #[serde(rename = "value")]
    pub name: String,
}

////////////////
// getArtists //
////////////////

#[derive(Debug, Deserialize)]
struct GetArtistsBody {
    #[serde(rename = "artists")]
    inner: GetIndexesInner,
}

////////////////
// getArtist //
////////////////

#[derive(Debug, Deserialize)]
struct GetArtistBody {
    #[serde(rename = "artist")]
    inner: GetArtistInner,
}

#[derive(Debug, Deserialize)]
struct GetArtistInner {
    #[serde(flatten)]
    artist_id3: ArtistId3,

    #[serde(rename = "album")]
    albums: Option<Vec<AlbumId3>>,
}

#[derive(Debug)]
pub struct ArtistWithAlbumsId3 {
    pub artist_id3: ArtistId3,
    pub albums: Vec<AlbumId3>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistId3 {
    pub id: String,
    pub name: String,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub album_count: Option<i32>,
    pub starred: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumId3 {
    pub id: String,
    pub name: String,
    pub artist: Option<String>,
    pub artist_id: Option<String>,
    pub cover_art: Option<String>,
    pub song_count: Option<i32>,
    pub duration: Option<i32>,
    pub play_count: Option<i32>,
    pub created: Option<String>,
    pub starred: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
}

//////////////
// getAlbum //
//////////////

#[derive(Debug, Deserialize)]
struct GetAlbumBody {
    #[serde(rename = "album")]
    inner: GetAlbumInner,
}

#[derive(Debug, Deserialize)]
struct GetAlbumInner {
    #[serde(flatten)]
    album_id3: AlbumId3,

    #[serde(rename = "song")]
    songs: Option<Vec<Child>>,
}

#[derive(Debug, Deserialize)]
pub struct AlbumWithSongsId3 {
    pub album_id3: AlbumId3,
    pub songs: Vec<Child>,
}

///////////////////
// getArtistInfo //
///////////////////

#[derive(Debug, Deserialize)]
struct GetArtistInfoBody {
    #[serde(rename = "artistInfo")]
    inner: GetArtistInfoInner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetArtistInfoInner {
    #[serde(flatten)]
    base: ArtistInfoBase,

    #[serde(rename = "similarArtist")]
    similar_artists: Option<Vec<Artist>>,
}

#[derive(Debug)]
pub struct ArtistInfo {
    pub base: ArtistInfoBase,
    pub similar_artists: Vec<Artist>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfoBase {
    pub biography: Option<String>,
    pub music_brainz_id: Option<String>,
    pub last_fm_url: Option<String>,
    pub small_image_url: Option<String>,
    pub medium_image_url: Option<String>,
    pub large_image_url: Option<String>,
}

///////////////////
// getArtistInfo2 //
///////////////////

#[derive(Debug, Deserialize)]
struct GetArtistInfo2Body {
    #[serde(rename = "artistInfo2")]
    inner: GetArtistInfo2Inner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetArtistInfo2Inner {
    #[serde(flatten)]
    base: ArtistInfoBase,

    #[serde(rename = "similarArtist")]
    similar_artists: Option<Vec<ArtistId3>>,
}

#[derive(Debug)]
pub struct ArtistInfo2 {
    pub base: ArtistInfoBase,
    pub similar_artists: Vec<ArtistId3>,
}

//////////////////
// getAlbumInfo //
//////////////////

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetAlbumInfoBody {
    album_info: AlbumInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfo {
    pub notes: Option<String>,
    pub music_brainz_id: Option<String>,
    pub last_fm_url: Option<String>,
    pub small_image_url: Option<String>,
    pub medium_image_url: Option<String>,
    pub large_image_url: Option<String>,
}

//////////////////
// getAlbumList //
//////////////////

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AlbumListType {
    Random,
    Newest,
    Highest,
    Frequent,
    Recent,
    AlphabeticalByName,
    AlphabeticalByArtist,
    Starred,
    ByYear { from_year: u16, to_year: u16 },
    ByGenre { genre: String },
}

impl AlbumListType {
    pub fn to_param_str(&self) -> &'static str {
        match self {
            AlbumListType::Random => "random",
            AlbumListType::Newest => "newest",
            AlbumListType::Highest => "highest",
            AlbumListType::Frequent => "frequent",
            AlbumListType::Recent => "recent",
            AlbumListType::AlphabeticalByName => "alphabeticalByName",
            AlbumListType::AlphabeticalByArtist => "alphabeticalByArtist",
            AlbumListType::Starred => "starred",
            AlbumListType::ByYear { .. } => "byYear",
            AlbumListType::ByGenre { .. } => "byGenre",
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetAlbumListBody {
    #[serde(rename = "albumList")]
    inner: GetAlbumListInner,
}

#[derive(Debug, Deserialize)]
struct GetAlbumListInner {
    album: Option<Vec<Child>>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[track_caller]
    fn check_example<T: DeserializeOwned>(path: &str) {
        let json = std::fs::read_to_string(path).unwrap();
        let res = serde_json::from_str::<ResponseEnvelope<T>>(&json);
        if let Err(e) = res {
            println!("{e}");
            assert!(false, "Json deserialization failed for {path}")
        }
    }

    #[test]
    fn deserialize_examples() {
        check_example::<PingBody>("test-data/navidrome/ping.json");
        check_example::<GetLicenseBody>("test-data/navidrome/getLicense.json");
        check_example::<GetMusicDirectoryBody>("test-data/navidrome/getMusicDirectory.json");
        check_example::<GetGenresBody>("test-data/navidrome/getGenres.json");
        check_example::<GetArtistsBody>("test-data/navidrome/getArtists.json");
        check_example::<GetArtistBody>("test-data/navidrome/getArtist.json");
        check_example::<GetAlbumBody>("test-data/navidrome/getAlbum.json");
        check_example::<GetArtistInfoBody>("test-data/navidrome/getArtistInfo.json");
        check_example::<GetArtistInfo2Body>("test-data/navidrome/getArtistInfo2.json");
    }
}

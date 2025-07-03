/*
{"id": "0000000000000000", "rightsId": null, "name": null, "isDLC": false, "isUpdate": false, "idExt": 0, "updateId": "0000000000000800", "version": null, "key": null, "isDemo": null, "region": null, "regions": null, "baseId": "0000000000000000", "releaseDate": null,
"nsuId": null, "category": null, "ratingContent": null, "numberOfPlayers": null, "rating": null, "developer": null, "publisher": null, "frontBoxArt": null, "iconUrl": null, "screenshots": null, "bannerUrl": null, "intro": null, "description": null, "languages": null, "size": 41896, "rank": null, "mtime": 1751395579.4308627}
*/

// complete game entry (Option - null for None -> valid)
#[derive(miniserde::Serialize, Default)]
pub struct GameEntry {
    id: String,
    rightsId: Option<String>,
    name: Option<String>,
    isDLC: bool,
    isUpdate: bool,
    idExt: u32,
    updateId: Option<String>,
    version: Option<String>,
    key: Option<String>,
    isDemo: Option<String>,
    region: Option<String>,
    regions: Option<String>,
    baseId: String,
    releaseDate: Option<String>,
    nsuId: Option<String>,
    category: Option<String>,
    ratingContent: Option<String>,
    numberOfPlayers: Option<u32>,
    rating: Option<String>,
    developer: Option<String>,
    publisher: Option<String>,
    frontBoxArt: Option<String>,
    iconUrl: Option<String>,
    screenshots: Option<String>,
    bannerUrl: Option<String>,
    intro: Option<String>,
    description: Option<String>,
    size: u64,
    rank: Option<String>,
    mtime: f64,
}

impl GameEntry {
    // don't support rich descriptions of custom nsps - I think just this is sufficient
    pub fn plain_new(id: String, size: u64, mtime: f64) -> Self {
        Self {
            updateId: Some(id.clone()),
            id,
            size,
            ..Default::default()
        }
    }
}

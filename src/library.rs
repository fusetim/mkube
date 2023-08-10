use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

#[cfg(feature = "ftp")]
use remotefs_ftp::client::FtpFs;
#[cfg(feature = "smb")]
use remotefs_smb::{SmbCredentials, SmbFs, SmbOptions};

use crate::localfs::LocalFs;
use crate::multifs::MultiFs;

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum LibraryType {
    #[default]
    Local,
    #[cfg(feature = "ftp")]
    Ftp,
    #[cfg(feature = "smb")]
    Smb,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum LibraryFlavor {
    Movie,
    TvShow,
}

impl LibraryType {
    pub fn to_scheme(&self) -> &'static str {
        match self {
            LibraryType::Local => "file",
            #[cfg(feature = "ftp")]
            LibraryType::Ftp => "ftp",
            #[cfg(feature = "smb")]
            LibraryType::Smb => "smb",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Library {
    pub fs_type: LibraryType,
    pub flavor: LibraryFlavor,
    pub name: String,
    pub host: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub path: PathBuf,
}

impl TryFrom<&Library> for Url {
    type Error = ();

    fn try_from(l: &Library) -> Result<Url, ()> {
        let scheme = l.fs_type.to_scheme();
        let mut url = Url::parse(&format!(
            "{}://{}{}",
            scheme,
            l.host.as_deref().unwrap_or(""),
            l.path.display()
        ))
        .map_err(|_| {})?;
        if url.has_host() {
            if let Some(user) = l.username.as_deref() {
                url.set_username(user)?;
            }
            url.set_password(l.password.as_deref())?;
        }
        Ok(url)
    }
}

impl TryFrom<&Library> for MultiFs {
    type Error = ();

    fn try_from(l: &Library) -> Result<MultiFs, ()> {
        match l.fs_type {
            LibraryType::Local => Ok(MultiFs::Local(LocalFs::new(l.path.clone()))),
            #[cfg(feature = "ftp")]
            LibraryType::Ftp => {
                if let Some(host) = &l.host {
                    let mut ftpfs = FtpFs::new(host, 21);
                    if let Some(username) = &l.username {
                        ftpfs = ftpfs.username(username);
                    }
                    if let Some(password) = &l.password {
                        ftpfs = ftpfs.password(password);
                    }
                    Ok(MultiFs::Ftp(ftpfs))
                } else {
                    Err(())
                }
            }
            #[cfg(feature = "smb")]
            LibraryType::Smb => {
                if let Some(host) = &l.host {
                    let mut crds = SmbCredentials::default().server(format!("smb://{}", host));
                    if let Some(username) = &l.username {
                        crds = crds.username(username);
                    }
                    if let Some(password) = &l.password {
                        crds = crds.password(password);
                    }
                    let opts = SmbOptions::default()
                        .case_sensitive(true)
                        .one_share_per_server(true);
                    SmbFs::try_new(crds, opts)
                        .map(|smb| MultiFs::Smb(smb))
                        .map_err(|_| {})
                } else {
                    Err(())
                }
            }
        }
    }
}

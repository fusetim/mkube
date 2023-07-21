use url::Url;
use std::path::PathBuf;
use std::str::FromStr;

use remotefs_ftp::client::FtpFs;

use crate::multifs::MultiFs;
use crate::localfs::LocalFs;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum LibraryType {
    #[default]
    Local,
    Ftp,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LibraryFlavor {
    Movie,
    TvShow,
}

impl LibraryType {
    pub fn to_scheme(&self) -> &'static str {
        match self {
            LibraryType::Local => "file",
            LibraryType::Ftp => "ftp",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
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
        let mut url = Url::parse(&format!("{}://{}{}", scheme, l.host.as_deref().unwrap_or(""), l.path.display()))
            .map_err(|_|{})?;
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
            LibraryType::Local => {
                Ok(MultiFs::Local(LocalFs::new(l.path.clone())))
            }, 
            LibraryType::Ftp => {
                let url = Url::try_from(l)?;
                if let Some(host) = url.host_str() {
                    if url.username().len() > 0{
                        if let Some(password) = url.password() {
                            let ftpfs = FtpFs::new(host, url.port_or_known_default().unwrap())
                                .username(url.username())
                                .password(password);
                            return Ok(MultiFs::Ftp(ftpfs));
                        }
                    } else {
                        let ftpfs = FtpFs::new(host, url.port_or_known_default().unwrap());
                        return Ok(MultiFs::Ftp(ftpfs));
                    }
                }
                Err(())
            }
        }
    }
}
use crate::localfs::LocalFs;
use remotefs::fs::RemoteFs;
use std::sync::{Mutex, Arc};
use std::path::PathBuf;
use std::str::FromStr;
use metadata::MediaFileMetadata;
use std::io::{Cursor, Read, Seek, Write, BufRead, Result as IoResult, self, SeekFrom};
use anyhow::{Result, anyhow};
use remotefs_ftp::client::FtpFs;
use remotefs_smb::{SmbFs};

pub enum MultiFs {
    Local(LocalFs),
    Ftp(FtpFs),
    Smb(SmbFs),
}

impl MultiFs {
    pub fn as_mut_rfs(&mut self) -> &mut dyn RemoteFs {
        match self {
            MultiFs::Local(lfs) => lfs,
            MultiFs::Ftp(ftp) => ftp,
            MultiFs::Smb(smb) => smb,
        }
    }


}

#[derive(Clone, Debug)]
pub struct OwnedCursor {
    inner: Arc<Mutex<Cursor<Vec<u8>>>>,
}

impl OwnedCursor {
    pub fn new() -> Self {
        Self{ inner: Arc::new(Mutex::new(Cursor::new(Vec::new()))) }
    }
}

impl Read for OwnedCursor {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self.inner.lock() {
            Ok(mut inner) => {
                (*inner).read(buf)
            },
            Err(_err) => { Err(io::Error::new(io::ErrorKind::Other, "Mutex failed!")) },
        }
    }
}

impl Write for OwnedCursor {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self.inner.lock() {
            Ok(mut inner) => {
                (*inner).write(buf)
            },
            Err(_err) => { Err(io::Error::new(io::ErrorKind::Other, "Mutex failed!")) },
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self.inner.lock() {
            Ok(mut inner) => {
                (*inner).flush()
            },
            Err(_err) => { Err(io::Error::new(io::ErrorKind::Other, "Mutex failed!")) },
        }
    }
}

impl Seek for OwnedCursor {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        match self.inner.lock() {
            Ok(mut inner) => {
                (*inner).seek(pos)
            },
            Err(_err) => { Err(io::Error::new(io::ErrorKind::Other, "Mutex failed!")) },
        }
    }
}

pub fn open_multifs_media(mfs: &mut dyn RemoteFs, mut ffmpeg_base: url::Url, path: PathBuf) -> Result<MediaFileMetadata> {
    use ffmpeg_next as ffmpeg;
    use ffmpeg::media::Type;
    use ffmpeg::util::rational::Rational;
    use std::fs;
    use std::io;
    use std::path::Path;
    use metadata::prejudice;
    use metadata::scan::{self, ScanType};
    use metadata::stream::{parse_stream_meatadata, StreamMetadata};
    use metadata::tags::{Tags, ToTags};
    use metadata::media_file::{StreamTags, MediaFileMetadataOptions};
    use metadata::util;

    let decoded_path = urlencoding::decode(ffmpeg_base.path())
        .map_err(|err| anyhow!("failed to decode path (url-encoding), causes:\n{:?}", err))?;
    let mut root = PathBuf::from_str(&decoded_path).unwrap();
    ffmpeg_base.set_path("/");
    root.push(&path);
    let ff_path = PathBuf::from_str(&format!("{}/{}", ffmpeg_base.to_string(), root.display())).unwrap();

    let mut format_ctx = ffmpeg::format::input(&ff_path)
        .map_err(|err| anyhow!("FFMpeg error: open failed for {}, causes:\n{:?}", ff_path.display(), err))?;

    let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
    let file_size = mfs.stat(&path).map_err(|err| anyhow!("Remotefs error: failed to read metadata {:?}", err))?.metadata.size;
    let file_size_base10 = util::human_size(file_size, util::Base::Base10);
    let file_size_base2 = util::human_size(file_size, util::Base::Base2);

    let container_format = prejudice::format_name(&format_ctx.format(), &path);

    let _duration = if format_ctx.duration() >= 0 {
        Some(format_ctx.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64)
    } else {
        None
    };
    let duration = _duration.map(util::format_seconds);

    let _scan_type = { scan::get_scan_type(&mut format_ctx)? };
    let scan_type = _scan_type.clone().map(|s| s.to_string());

    let _bit_rate = match format_ctx.bit_rate() {
        0 => None,
        _ => Some(format_ctx.bit_rate() as u64),
    };
    let bit_rate = if let Some(rate) = _bit_rate {
        Some(format!("{:.0} kb/s", rate as f64 / 1000f64))
    } else if let Some(seconds) = _duration {
        if seconds > 0f64 {
            Some(format!(
                "{:.0} kb/s",
                (file_size * 8) as f64 / seconds / 1000f64
            ))
        } else {
            None
        }
    } else {
        None
    };

    let mut _streams_metadata = Vec::new();
    for stream in format_ctx.streams() {
        _streams_metadata.push(parse_stream_meatadata(stream)?);
    }
    let streams_metadata_rendered = _streams_metadata
        .iter()
        .map(|m| {
            m.render_default().unwrap_or_else(|_| {
                panic!("failed to render metadata for stream #{}", m.index())
            })
        })
        .collect::<Vec<_>>();

    let best_vstream_index = format_ctx.streams().best(Type::Video).map(|s| s.index());
    let best_vstream_metadata =
        best_vstream_index.map(|i| _streams_metadata[i].video_metadata().unwrap());
    let (
        width,
        height,
        pixel_dimensions,
        _sample_aspect_ratio,
        sample_aspect_ratio,
        _display_aspect_ratio,
        display_aspect_ratio,
        _frame_rate,
        frame_rate,
    ) = if let Some(m) = best_vstream_metadata {
        (
            Some(m.width),
            Some(m.height),
            Some(m.pixel_dimensions),
            Some(m._sample_aspect_ratio),
            Some(m.sample_aspect_ratio),
            Some(m._display_aspect_ratio),
            Some(m.display_aspect_ratio),
            m._frame_rate,
            m.frame_rate,
        )
    } else {
        (None, None, None, None, None, None, None, None, None)
    };

    let tagdict = format_ctx.metadata();
    let title = tagdict
        .get("title")
        .or_else(|| tagdict.get("TITLE"))
        .map(|s| s.to_string());

    let tags = tagdict.to_tags();
    let filtered_tags = tagdict.to_filtered_tags();

    let streams_tags = format_ctx
        .streams()
        .map(|s| StreamTags {
            index: s.index(),
            tags: s.metadata().to_tags(),
        })
        .collect();
    let streams_filtered_tags = format_ctx
        .streams()
        .map(|s| StreamTags {
            index: s.index(),
            tags: s.metadata().to_filtered_tags(),
        })
        .collect();

    Ok(MediaFileMetadata {
        options: MediaFileMetadataOptions {
            include_checksum: false,
            include_tags: false,
            include_all_tags: false,
            decode_frames: false,
        },
        path: path.to_str().unwrap().to_string(),
        file_name,
        file_size,
        file_size_base10,
        file_size_base2,
        hash: None,
        title,
        container_format,
        _duration,
        duration,
        width,
        height,
        pixel_dimensions,
        _sample_aspect_ratio,
        sample_aspect_ratio,
        _display_aspect_ratio,
        display_aspect_ratio,
        _scan_type,
        scan_type,
        _frame_rate,
        frame_rate,
        _bit_rate,
        bit_rate,
        _streams_metadata,
        streams_metadata_rendered,
        tags,
        filtered_tags,
        streams_tags,
        streams_filtered_tags,
    })
}

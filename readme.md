# MKube - Minimalist Media Manager

*Yet another media manager, but better (?).*

MKube aims to be a complete media manager in your terminal. 

## State of development

Currently MKube is at most a PoC, and a lot of changes awaits it. 
Nonetheless, here a small list of the features, MKube is expected to provide
today or in the future.

- [x] Local libraries
- [x] Remote libraries (currently supporting FTP & SMB)
- [x] Movie listing
- [x] Movie search (using TheMovieDB)
- [x] Metadata retrieval (using ffmpeg)
- [x] NFO generation (for Kodi only)
- [ ] Movie details 
- [ ] Artwork retrievals
- [ ] TV Show support
- [ ] Persistent configuration
- [ ] Conditional features

## Build & Run

MKube is written in Rust and currently built using Nix (to allow easy build on 
multiple arch).

### Using Rust

At the moment, MKube does not inplement any condition features, therefore you 
will need to install every system dependencies required by Mkube and its dependencies.
This includes (but might not be limited to):
- ffmpeg(-dev)
- libsmbclient
- openssl

Then, just build it: `cargo build --release`

Note: You might need to install a recent (or nightly) rust toolchain to build MKube.
At the time of writting, MKube targets Rust `nightly-2023-07-27`.

### Using Nix

MKube use Nix Flakes to describe its build steps, therefore you might need to 
enable flakes (experimental-features) before building MKube using 
`nix build`.

Note: For developement, you should prefer to run `nix develop` to create a shell 
adapted to rust development.

## Licencing

***To Be Done***
*If needed, please contact me. I have not figure this out for the moment but certainly will licence this software as GPLv3*

Fusetim (2023) - All rights reserved.
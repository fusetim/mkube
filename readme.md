# MKube - Minimalist Media Manager

*Yet another media manager, but better (?).*

MKube aims to be a complete (and free) media manager in your terminal. 

## State of development

Currently MKube is at most a PoC, and lots of changes await it. 
Nonetheless, here a small list of features, MKube is expected to provide
today or in the future :

- [x] Local libraries
- [x] Remote libraries (currently supporting FTP & SMB)
- [x] Movie listing
- [x] Movie search (using TheMovieDB)
- [x] Metadata retrieval (using ffmpeg)
- [x] NFO generation (for Kodi only)
- [x] Persistent configuration
- [x] Movie details 
- [ ] Movie Editor
- [ ] Artwork retrievals
- [ ] TV Show support

## Build & Run

MKube is written in Rust and currently built using Nix (to allow easy build on 
multiple arch).

### Supported platforms

Until a first release, MKube will not be tested on any other machine than mine,
moreover the `main` branch might break from time to time. 

My long term strategy is to support mainly Linux platforms (`amd64`, `arm64`).  
MacOS might work (as dependencies at least exist there) but it will not be tested.  
I am unsure if Windows will get supported one day, even partially.

### Using Cargo

MKube currently requires the following dependencies (but it might not be limited to these):
- ffmpeg(-dev)
- libsmbclient (`smb` feature, *enabled by default*)
- openssl

Then, just build it: `cargo build --release`

Note: You might need to install a recent (or nightly) rust toolchain to build MKube.
At the time of writting, MKube targets Rust `nightly-2023-07-27`.

### Using Nix

MKube uses Nix Flakes to describe its build steps, therefore you might need to 
enable flakes (experimental-features) before building MKube using 
`nix build`.

Note: For developement, you should prefer to run `nix develop` to create a shell 
adapted to rust development.

## Showcase

![mkube](/docs/mkube_demo_2023-08-10.png)

## Inspirations
The Linux community is lacking a good media manager, that can support remote libraries.
Some good media managers exist, like [tinyMediaManager](https://tinymediamanager.org) but 
lack features or enforce to buy premium version to access all their features, including 
some essential ones. MKube is an attempt to resolve this issue I had myself.

## Licensing

Licensed under the **EUPL v. 1.2 only**.  
*Note that the EUPL v1.2 license is compatible with many other recognized licenses (see appendix).*

Fusetim (2023) - All rights reserved.
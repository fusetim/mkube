{
    pkgs,
    craneLib,
    rust-toolchain,

    ffmpeg ? pkgs.ffmpeg,
    openssl ? pkgs.openssl,
    dbus ? pkgs.dbus,
    samba ? pkgs.samba,

    enable-secrets ? true,
    enable-smb ? false,
    enable-ftp ? true,
}:

let
    inherit (pkgs) lib;
    src = craneLib.cleanCargoSource (craneLib.path ./.);

    commonArgs = {
        inherit src;
        nativeBuildInputs = [pkgs.pkg-config];
        buildInputs =  [
            (rust-toolchain.withComponents [
              "cargo"
              "rustc"
            ])
            ffmpeg
            ffmpeg.dev
            ffmpeg.lib
            pkgs.llvmPackages_latest.libclang
            pkgs.rustPlatform.bindgenHook
            openssl
          ] ++ pkgs.lib.optionals enable-secrets [
            dbus.lib
          ] ++ pkgs.lib.optionals enable-smb [
            samba
            samba.dev
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];
    };

    craneLibLLvmTools = craneLib.overrideToolchain (rust-toolchain.withComponents [
        "cargo"
        "llvm-tools"
        "rustc"
    ]);

    features = []   ++ pkgs.lib.optional enable-secrets "secrets"
                    ++ pkgs.lib.optional enable-ftp "ftp"
                    ++ pkgs.lib.optional enable-smb "smb";

    cargoArtifacts = craneLib.buildDepsOnly commonArgs;

in craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = commonArgs.buildInputs ++ [];
        cargoExtraArgs = "--no-default-features --features ${builtins.concatStringsSep "," features}";
        meta = with lib; {
            description = "Minimalist Media Manager (M³) - Rust minimalist TUI to manage your remote mediacenter.";
            longDescription = ''
              Minimalist Media Manager (M³) - Rust minimalist TUI to manage your remote mediacenter.
            '';
            homepage = "https://github.com/fusetim/mkube/";
            license = if enable-smb then licenses.eupl12 else licenses.gpl3Plus;
        };
    })
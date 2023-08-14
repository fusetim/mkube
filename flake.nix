{
  description = "Minimalist Media Manager (mkube)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };

  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let 
        features = [ "smb" "ftp" "secrets" ];
        toolchain = (fenix.packages.${system}.toolchainOf {
            channel = "1.71.0";
            sha256 = "sha256-ks0nMEGGXKrHnfv4Fku+vhQ7gx76ruv6Ij4fKZR3l78=";
          });
      in
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        inherit (pkgs) lib;
        inherit toolchain;
        inherit features;

        craneLib = crane.lib.${system};
        src = craneLib.cleanCargoSource (craneLib.path ./.);

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;

          nativeBuildInputs = [pkgs.pkg-config];

          buildInputs = [
            (toolchain.withComponents [
              "cargo"
              "rustc"
            ])
            pkgs.ffmpeg
            pkgs.ffmpeg.dev
            pkgs.ffmpeg.lib
            pkgs.llvmPackages_latest.libclang
            pkgs.rustPlatform.bindgenHook
            pkgs.openssl
          ] ++ lib.optionals (builtins.elem "secrets" features) [
            # Additional darwin specific inputs can be set here
            pkgs.dbus.lib
          ] ++ lib.optionals (builtins.elem "smb" features) [
            # Additional darwin specific inputs can be set here
            pkgs.samba
            pkgs.samba.dev
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];

        };

        craneLibLLvmTools = craneLib.overrideToolchain (toolchain.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        my-crate = craneLib.buildPackage (commonArgs // {
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
            license = licenses.eupl12;
          };
        });
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit my-crate;

          # Run clippy (and deny all warnings) on the crate source,
          # again, resuing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          my-crate-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          my-crate-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          # Check formatting
          my-crate-fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Audit dependencies
          my-crate-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # Run tests with cargo-nextest
          # Consider setting `doCheck = false` on `my-crate` if you do not want
          # the tests to run twice
          my-crate-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
        } // lib.optionalAttrs (system == "x86_64-linux") {
          # NB: cargo-tarpaulin only supports x86_64 systems
          # Check code coverage (note: this will not upload coverage anywhere)
          my-crate-coverage = craneLib.cargoTarpaulin (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        packages = {
          default = my-crate;
          my-crate-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        apps.default = flake-utils.lib.mkApp {
          drv = my-crate;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

          # Extra inputs can be added here
          nativeBuildInputs = with pkgs; [
            (toolchain.withComponents [
              "cargo"
              "rustc"
            ])
            pkg-config
          ];

          buildInputs = commonArgs.buildInputs ++ [ ];

          LIBCLANG_PATH = "${pkgs.llvmPackages_latest.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = 
              # Includes with normal include path
              (builtins.map (a: ''-I"${a}/include"'') [
                # add dev libraries here (e.g. pkgs.libvmi.dev)
                pkgs.glibc.dev 
                pkgs.ffmpeg.dev
                pkgs.samba.dev
                pkgs.dbus.dev
              ])
              # Includes with special directory paths
              ++ [
                ''-I"${pkgs.llvmPackages_latest.libclang.lib}/lib/clang/${pkgs.llvmPackages_latest.libclang.version}/include"''
                ''-I"${pkgs.glib.dev}/include/glib-2.0"''
                ''-I${pkgs.glib.out}/lib/glib-2.0/include/''
              ];
        };
      });
}

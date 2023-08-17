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
        features = [ "ftp" "secrets" ];
        toolchain = (fenix.packages.${system}.toolchainOf {
            channel = "1.71.0";
            sha256 = "sha256-ks0nMEGGXKrHnfv4Fku+vhQ7gx76ruv6Ij4fKZR3l78=";
          });
      in
      let
        inherit toolchain features system;
        pkgs = import nixpkgs {
          inherit system;
        };

        getBuildInputs = {pkgs, toolchain, features}: [
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
          ] ++ pkgs.lib.optionals (builtins.elem "secrets" features) [
            pkgs.dbus.lib
          ] ++ pkgs.lib.optionals (builtins.elem "smb" features) [
            pkgs.samba
            pkgs.samba.dev
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];


        hostBuild = {pkgs, system, features, toolchain, ...}: rec {
          inherit (pkgs) lib;

          craneLib = crane.lib.${system};
          src = craneLib.cleanCargoSource (craneLib.path ./.);

          commonArgs = {
            inherit src;
            nativeBuildInputs = [pkgs.pkg-config];
            buildInputs = (getBuildInputs { inherit pkgs toolchain features; });
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
          mkube-bin = craneLib.buildPackage (commonArgs // {
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

          # Run Clippy on mkube
          mkube-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          # Doc generation
          mkube-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          # Check formatting
          mkube-fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Audit dependencies
          mkube-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # LLVM coverage
          mkube-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
        };
      in {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit (hostBuild { inherit pkgs system features toolchain; }) mkube-bin mkube-clippy mkube-doc mkube-fmt mkube-audit;
        };

        packages = {
          default = (hostBuild { inherit pkgs system features toolchain; }).mkube-bin;
          mkube-llvm-coverage = (hostBuild { inherit pkgs system features toolchain; }).mkube-llvm-coverage;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = (hostBuild { inherit pkgs system features toolchain; }).mkube-bin;
        };

        lib = {
          mkube-hostBuild = hostBuild;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.checks.${system};

          # Extra inputs can be added here
          nativeBuildInputs = with pkgs; [
            (toolchain.withComponents [
              "cargo"
              "rustc"
            ])
            pkg-config
          ];

          buildInputs = (getBuildInputs { 
            inherit pkgs toolchain;
            features = [ "smb" "ftp" "secrets" ];
          });

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
      } 
    );
}
     
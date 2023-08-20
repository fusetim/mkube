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
        rust-toolchain = (fenix.packages.${system}.toolchainOf {
            channel = "1.71.0";
            sha256 = "sha256-ks0nMEGGXKrHnfv4Fku+vhQ7gx76ruv6Ij4fKZR3l78=";
          });
      in
      let
        inherit rust-toolchain system;
        pkgs = import nixpkgs {
          inherit system;
        };
        craneLib = crane.lib.${system};
      in {
        checks = {
          mkube-bin = pkgs.callPackage ./mkube.nix { inherit pkgs craneLib rust-toolchain; };
        };

        packages = {
          default = pkgs.callPackage ./mkube.nix { inherit pkgs craneLib rust-toolchain; };
        };

        apps.default = flake-utils.lib.mkApp {
          drv = pkgs.callPackage ./mkube.nix { inherit pkgs craneLib rust-toolchain; };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.checks.${system};

          # Extra inputs can be added here
          nativeBuildInputs = with pkgs; [
            (rust-toolchain.withComponents [
              "cargo"
              "rustc"
            ])
            pkg-config
          ];

          buildInputs = [
            (rust-toolchain.withComponents [
              "cargo"
              "rustc"
            ])
            pkgs.ffmpeg
            pkgs.ffmpeg.dev
            pkgs.ffmpeg.lib
            pkgs.llvmPackages_latest.libclang
            pkgs.rustPlatform.bindgenHook
            pkgs.openssl
            pkgs.dbus.lib
            pkgs.samba
            pkgs.samba.dev
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

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
     
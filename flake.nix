{
  inputs = {
    nixpkgs.url = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    vcube = {
      url = "github:qter-project/vcube";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    tytanic = {
      url = "github:typst-community/tytanic/v0.4.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      vcube,
      tytanic,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rust = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
          targets = [
            "aarch64-unknown-linux-gnu"
            "wasm32-unknown-unknown"
          ];
        };

        libraries = with pkgs; [
          udev
          alsa-lib-with-plugins
          vulkan-loader
          libx11
          libxcursor
          libxi
          libxrandr # To use the x11 feature
          libxkbcommon
          wayland # To use the wayland feature
        ];

        # Used in typst documents
        fontsConf = pkgs.makeFontsConf {
          fontDirectories = with pkgs; [
            martian-mono
            monaspace
          ];
        };
      in
      rec {
        toolchain = ./rust-toolchain.toml;

        devShell = pkgs.mkShell rec {
          buildInputs =
            libraries
            ++ (with pkgs; [
              sccache
              rust-analyzer
              rust
              pkg-config
              tree-sitter
              packages.rob-twophase
              packages.shiroa
              vcube.defaultPackage."${system}"
              caddy
              nodejs
              typescript
              wasm-pack
              cargo-nextest
              tytanic.packages."${system}".default

              # (gap.overrideAttrs (o: {
              #   version = "4.13.1";
              #   patches = [ ];
              #   src = fetchurl {
              #     url = "https://github.com/gap-system/gap/releases/download/v4.13.1/gap-4.13.1.tar.gz";
              #     sha256 = "sha256-l5Tb26b7mY4KLQqoziH8iEitPT+cyZk7C44gvn4dvro=";
              #   };
              # }))
              gap

              (python3.withPackages (
                p: with p; [
                  sympy
                ]
              ))
              python312Packages.python-lsp-server
            ]);

          RUST_BACKTRACE = 1;
          RUSTC_WRAPPER = "sccache";
          SCCACHE_SERVER_PORT = "54226";
          # RUSTFLAGS = "-C target-cpu=native";
          FONTCONFIG_FILE = "${fontsConf}";

          # CFLAGS_wasm32_unknown_unknown = "-mno-reference-types";

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

          shellHook = ''
            export PATH=$PATH:~/.cargo/bin
          '';
        };

        packages.rob-twophase = pkgs.stdenv.mkDerivation {
          name = "rob-twophase";
          src = pkgs.fetchFromGitHub {
            owner = "efrantar";
            repo = "rob-twophase";
            rev = "d245031257d52b2663c5790c5410ef30aefd775f";
            hash = "sha256-2QZgW7w80+oKlMFMkIvuEXdp0SkIXpLs02MHe9qjb/c=";
          };
          buildPhase = ''
            make
          '';
          installPhase = ''
            mkdir -p $out/bin
            cp twophase $out/bin
          '';
        };

        packages.shiroa = pkgs.rustPlatform.buildRustPackage {
          pname = "shiroa";
          version = "0.4.0";

          src = pkgs.fetchFromGitHub {
            owner = "Myriad-Dreamin";
            repo = "shiroa";
            rev = "16efd7d1b7b01005a23b35278c66bcbef88b25dc";
            fetchSubmodules = true;
            sha256 = "sha256-6wrr9aVWp0QDcAd4T59WKtwZBZemdMxuNWbfDN8V8v8=";
          };

          cargoHash = "sha256-BWO49yMTCGtWTR/1u3/pvU/JdQi+2LlqjJX6p1fAUT0=";

          meta = {
            description = "A simple tool for creating modern online books in pure typst.";
            homepage = "https://github.com/Myriad-Dreamin/shiroa";
            license = pkgs.lib.licenses.asl20;
          };
        };

        robot-deps = [ packages.rob-twophase ];

        legacyPackages = packages;
      }
    );
}

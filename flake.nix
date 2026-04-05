{
  description = "recursive grep with boolean constraints";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, nixpkgs, fenix }:
    let
      overlay = final: prev: {
        resharp = final.callPackage ./resharp.nix { };
      };
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f {
            inherit system;
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );

      distTargets = [
        { rust = "x86_64-unknown-linux-gnu";  out = "re-x86_64-linux"; }
        { rust = "aarch64-unknown-linux-musl"; out = "re-aarch64-linux"; }
        { rust = "aarch64-apple-darwin";       out = "re-aarch64-macos"; }
        { rust = "x86_64-pc-windows-gnu";      out = "re-x86_64-windows.exe"; }
      ];
    in
    {
      overlays.default = overlay;

      packages = forAllSystems (
        { pkgs, system }:
        let
          fenixPkgs = fenix.packages.${system};
          toolchain = fenixPkgs.combine ([
            fenixPkgs.stable.cargo
            fenixPkgs.stable.rustc
          ] ++ map (t: fenixPkgs.targets.${t.rust}.stable.rust-std) distTargets);
          vendored = pkgs.rustPlatform.importCargoLock {
            lockFile = ./Cargo.lock;
          };
        in
        {
          default = pkgs.resharp;

          dist = pkgs.stdenv.mkDerivation {
            HOME = "/build";
            name = "re-dist";
            src = pkgs.lib.cleanSource ./.;
            nativeBuildInputs = [ toolchain pkgs.zig pkgs.cargo-zigbuild ];

            configurePhase = ''
              mkdir -p .cargo
              cat > .cargo/config.toml << EOF
[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "${vendored}"
EOF
            '';

            buildPhase =
              builtins.concatStringsSep "\n"
                (map (t: "cargo zigbuild --release --target ${t.rust}") distTargets);

            installPhase = ''
              mkdir -p $out
            '' + builtins.concatStringsSep "\n" (map (t:
              let ext = if pkgs.lib.hasSuffix ".exe" t.out then ".exe" else "";
              in "cp target/${t.rust}/release/re${ext} $out/${t.out}"
            ) distTargets);
          };
        }
      );

      devShells = forAllSystems (
        { pkgs, ... }:
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              rustc
              clippy
              rustfmt
              ripgrep
              hyperfine
            ];
          };
        }
      );
    };
}

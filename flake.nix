{
  description = "Flake for backend/frontend development";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };
  outputs = inputs @ {flake-parts, ...}: let
  in
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [];
      systems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin"];

      perSystem = {pkgs, ...}: rec {
        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.just
            pkgs.gnumake
            pkgs.pkg-config
            pkgs.openssl
            pkgs.sqlfluff
            pkgs.sqlx-cli
            pkgs.trunk
            pkgs.pre-commit
          ];
          shellHook = ''
            export XDG_CACHE_HOME="$(mktemp -d)"
            export DATABASE_URL="postgres://postgres:@localhost/bast3st"
            export BAST3ST_CONFIG="bast3st.toml"
            export RUST_LOG="warn"
          '';
        };
        packages.default = packages.bast3st-backend;
        packages.bast3st-backend = pkgs.rustPlatform.buildRustPackage {
          name = "bast3st-backend";
          src = ./.;
          buildInputs = [];
          nativeBuildInputs = [pkgs.pkg-config];
          cargoHash = "sha256-SGDUY8LQp4DrXqHfHiBT+XQ13d4xag7iCl3NIbl09kE=";
        };

        formatter = pkgs.alejandra;
      };
    };
}

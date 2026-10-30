{
  description = "mdvi terminal Markdown viewer";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "aarch64-linux" "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          mdvi = pkgs.rustPlatform.buildRustPackage {
            pname = manifest.package.name;
            version = manifest.package.version;
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./src
                ./xtask
                ./README.md
                ./LICENSE
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isx86_64 [ pkgs.nasm ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];

            cargoBuildFlags = [ "--package" "mdvi" ];
            cargoTestFlags = [ "--package" "mdvi" ];

            meta = {
              inherit (manifest.package) description;
              homepage = "https://github.com/nikvdp/mdvi";
              license = pkgs.lib.licenses.mit;
              mainProgram = "mdvi";
              platforms = systems;
            };
          };
        in
        {
          inherit mdvi;
          default = mdvi;
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          meta = self.packages.${system}.mdvi.meta;
          program = "${self.packages.${system}.mdvi}/bin/mdvi";
        };
      });
    };
}

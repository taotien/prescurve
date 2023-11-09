{
  inputs = {
    cargo2nix.url = "github:cargo2nix/cargo2nix/release-0.11.0";
    flake-utils.follows = "cargo2nix/flake-utils";
    nixpkgs.follows = "cargo2nix/nixpkgs";
  };

  outputs = inputs: with inputs;
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ cargo2nix.overlays.default ];
        };

        rustPkgs = pkgs.rustBuilder.makePackageSet {
          rustVersion = "1.70.0";
          packageFun = import ./Cargo.nix;
        };

      in
      rec {
        packages = {
          x86_64-linux.prescurve = (rustPkgs.workspace.prescurve { });
          x86_64-linux.default = packages.x86_64-linux.prescurve;
          # replace hello-world with your package name
          prescurve = (rustPkgs.workspace.prescurve { });
          default = packages.prescurve;
        };
        apps = rec {
          prescurve-backlight = { type = "app"; program = "${packages.default}/bin/cargo2nix"; };
          default = prescurve-backlight;
        };
        nixosModules = {
          prescurve-backlight = { config }: { };
        };
      }
    );
}

{
  description = "QterOS Config";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    nixpkgs-old.url = "github:NixOS/nixpkgs/nixos-24.11";
    nixpkgs-unstable.url = "github:nixos/nixpkgs/nixos-unstable";

    nixos-hardware.url = "github:NixOS/nixos-hardware/master";

    home-manager.url = "github:nix-community/home-manager/release-25.05";
    home-manager.inputs.nixpkgs.follows = "nixpkgs";
    
    agenix.url = "github:ryantm/agenix";

    qter.url = "path:../..";
    qter.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nixpkgs-unstable, nixpkgs-old, nixpkgs, nixos-hardware, home-manager, ... } @inputs: let
    defaultModules = system: [
      home-manager.nixosModules.home-manager
      {
        nixpkgs.overlays = [
          (final: _prev: {
            unstable = import nixpkgs-unstable {
              system = system;
            };
            nixpkgs-old = import nixpkgs-old {
              system = system;
            };
          })
        ];
      }
    ];
  in {
    nixosConfigurations = {
      rpi = nixpkgs.lib.nixosSystem {
        system = "aarch64-linux";
        specialArgs = { inherit inputs; };
        modules = (defaultModules "aarch64-linux") ++ [
          ./rpi.nix
          nixos-hardware.nixosModules.raspberry-pi-4
        ];
      };
    };
  };
}
